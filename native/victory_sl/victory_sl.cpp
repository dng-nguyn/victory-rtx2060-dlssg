// victory_sl.dll: a small C bridge between VIctory (Rust) and NVIDIA Streamline
// (C++), for DLSS Frame Generation on VIctory's own swap chain.
//
// VIctory owns the D3D12 device, queue and swap chain, so this uses
// Streamline's manual hooking: the queue is made through Streamline's proxy
// device and the swap chain through its proxy factory; everything else stays
// native. Each frame VIctory hands over the title's depth and motion vectors
// (copied by Streamline when tagged), the camera constants it has, and the
// Reflex/PCL markers DLSS-G requires.
//
// Built against the Streamline SDK headers and sl.interposer.lib
// (see build.ps1). Over-the-air plugin downloads are off.

#include <windows.h>
#include <d3d12.h>
#include <dxgi1_6.h>
#include <cmath>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <share.h>

#include <sl.h>
#include <sl_consts.h>
#include <sl_hooks.h>
#include <sl_dlss_g.h>
#include <sl_reflex.h>
#include <sl_pcl.h>

#define VSL_API extern "C" __declspec(dllexport)

namespace {

std::mutex g_lock;
bool g_ready = false;
sl::FrameToken* g_frame = nullptr;
uint32_t g_frameIndex = 0;
int g_framesToGenerate = 0;
sl::ViewportHandle g_viewport{0};
FILE* g_log = nullptr;

void logf(const char* fmt, ...) {
    if (!g_log) return;
    va_list args;
    va_start(args, fmt);
    vfprintf(g_log, fmt, args);
    va_end(args);
    fputc('\n', g_log);
    fflush(g_log);
}

void slLog(sl::LogType type, const char* msg) {
    logf("[sl %d] %s", (int)type, msg);
}

sl::float4x4 identity() {
    sl::float4x4 m{};
    m.row[0] = {1, 0, 0, 0};
    m.row[1] = {0, 1, 0, 0};
    m.row[2] = {0, 0, 1, 0};
    m.row[3] = {0, 0, 0, 1};
    return m;
}

// A left-handed perspective projection (row-vector convention, as SL wants),
// from the field of view the title gave its upscaler. With `inverted` the
// depth runs far=0, near=1 (reversed Z).
sl::float4x4 perspective(float fovY, float aspect, float nearZ, float farZ, bool inverted) {
    float y = 1.0f / std::tan(fovY * 0.5f);
    float x = y / aspect;
    float n = inverted ? farZ : nearZ;
    float f = inverted ? nearZ : farZ;
    if (!std::isfinite(f) || f <= 0.0f) f = 1.0e6f;
    float a = f / (f - n);
    sl::float4x4 m{};
    m.row[0] = {x, 0, 0, 0};
    m.row[1] = {0, y, 0, 0};
    m.row[2] = {0, 0, a, 1};
    m.row[3] = {0, 0, -n * a, 0};
    return m;
}

}  // namespace

// What VIctory knows of a frame.
struct VslFrame {
    void* depth;        // ID3D12Resource*
    uint32_t depthState;  // D3D12_RESOURCE_STATES
    void* mvec;
    uint32_t mvecState;
    uint32_t renderWidth, renderHeight;    // where depth/mvec are valid
    uint32_t outputWidth, outputHeight;    // the back buffer
    float jitterX, jitterY;                // in render pixels
    float mvecScaleX, mvecScaleY;          // to bring motion vectors to [-1,1]
    float nearZ, farZ, fovY;
    int32_t depthInverted;
    int32_t reset;
};

// 0 = ok. `pluginDir`: where sl.*.dll and nvngx_dlssg.dll are. `logPath`:
// a text log of this bridge and of Streamline (null: none).
VSL_API int vsl_init(const wchar_t* pluginDir, const wchar_t* logPath) {
    std::lock_guard<std::mutex> guard(g_lock);
    if (logPath && !g_log) g_log = _wfsopen(logPath, L"w", _SH_DENYNO);
    sl::Preferences pref{};
    pref.showConsole = false;
    pref.logLevel = sl::LogLevel::eDefault;
    pref.logMessageCallback = slLog;
    const wchar_t* paths[] = {pluginDir};
    pref.pathsToPlugins = paths;
    pref.numPathsToPlugins = 1;
    // Manual hooking; no over-the-air downloads.
    pref.flags = sl::PreferenceFlags::eDisableCLStateTracking | sl::PreferenceFlags::eUseManualHooking;
    sl::Feature features[] = {sl::kFeatureDLSS_G, sl::kFeatureReflex, sl::kFeaturePCL};
    pref.featuresToLoad = features;
    pref.numFeaturesToLoad = 3;
    // No NVIDIA application id: NGX (which DLSS-G needs) then wants the
    // engine's version and a project GUID, or it disables every NGX feature
    // ("Please provide correct application id when calling slInit").
    pref.engine = sl::EngineType::eCustom;
    pref.engineVersion = "1.0.0";
    pref.projectId = "c6a3dbdc-bb28-4a3e-8316-a32bd8a3df73";
    pref.renderAPI = sl::RenderAPI::eD3D12;
    sl::Result r = slInit(pref);
    logf("slInit -> %d", (int)r);
    return (int)r;
}

VSL_API int vsl_set_device(void* device) {
    sl::Result r = slSetD3DDevice(device);
    logf("slSetD3DDevice -> %d", (int)r);
    return (int)r;
}

// Makes the queue through Streamline's proxy device (a mandatory hook).
VSL_API int vsl_create_queue(void* nativeDevice, const D3D12_COMMAND_QUEUE_DESC* desc, void** queue) {
    ID3D12Device* proxy = static_cast<ID3D12Device*>(nativeDevice);
    proxy->AddRef();
    sl::Result r = slUpgradeInterface(reinterpret_cast<void**>(&proxy));
    if (r != sl::Result::eOk) {
        proxy->Release();
        logf("slUpgradeInterface(device) -> %d", (int)r);
        return (int)r;
    }
    HRESULT hr = proxy->CreateCommandQueue(desc, __uuidof(ID3D12CommandQueue), queue);
    logf("proxy CreateCommandQueue -> %08x", (unsigned)hr);
    return SUCCEEDED(hr) ? 0 : (int)hr;
}

// Replaces `*object` (an IDXGIFactory* or IDXGISwapChain*) with its proxy.
VSL_API int vsl_upgrade(void** object) {
    sl::Result r = slUpgradeInterface(object);
    logf("slUpgradeInterface -> %d", (int)r);
    return (int)r;
}

// Whether DLSS-G may run on the adapter with this LUID; `maxGenerated`: the
// most frames it may generate per real one (0 if unsupported).
VSL_API int vsl_check(uint32_t luidLow, int32_t luidHigh, int* maxGenerated) {
    LUID luid{luidLow, luidHigh};
    sl::AdapterInfo info{};
    info.deviceLUID = reinterpret_cast<uint8_t*>(&luid);
    info.deviceLUIDSizeInBytes = sizeof(LUID);
    sl::Result r = slIsFeatureSupported(sl::kFeatureDLSS_G, info);
    logf("slIsFeatureSupported(DLSS_G) -> %d", (int)r);
    *maxGenerated = 0;
    sl::DLSSGState state{};
    if (slDLSSGGetState(g_viewport, state, nullptr) == sl::Result::eOk) {
        *maxGenerated = (int)state.numFramesToGenerateMax;
        logf("DLSS-G state: status %u, max frames to generate %u, min width/height %u",
             (unsigned)state.status, (unsigned)state.numFramesToGenerateMax,
             (unsigned)state.minWidthOrHeight);
    }
    return (int)r;
}

// Frame generation on (`frames` generated per real frame) or off (0).
VSL_API int vsl_set_options(int frames, uint32_t width, uint32_t height) {
    sl::DLSSGOptions o{};
    o.mode = frames > 0 ? sl::DLSSGMode::eOn : sl::DLSSGMode::eOff;
    o.numFramesToGenerate = frames > 0 ? (uint32_t)frames : 1;
    o.colorWidth = width;
    o.colorHeight = height;
    sl::Result r = slDLSSGSetOptions(g_viewport, o);
    logf("slDLSSGSetOptions(frames %d) -> %d", frames, (int)r);
    g_framesToGenerate = frames;
    return (int)r;
}

// Reflex must run for DLSS-G: low latency mode on, no frame cap.
VSL_API int vsl_reflex_on() {
    sl::ReflexOptions o{};
    o.mode = sl::ReflexMode::eLowLatency;
    sl::Result r = slReflexSetOptions(o);
    logf("slReflexSetOptions -> %d", (int)r);
    return (int)r;
}

// The start of a frame: a new token and the simulation markers.
VSL_API int vsl_frame_begin() {
    sl::Result r = slGetNewFrameToken(g_frame, nullptr);
    if (r != sl::Result::eOk || !g_frame) return (int)r;
    g_frameIndex = *g_frame;
    slPCLSetMarker(sl::PCLMarker::eSimulationStart, *g_frame);
    slPCLSetMarker(sl::PCLMarker::eSimulationEnd, *g_frame);
    slPCLSetMarker(sl::PCLMarker::eRenderSubmitStart, *g_frame);
    return 0;
}

// Constants and tags for the frame, recorded on `cmdList` (Streamline copies
// the tagged resources there: they are only valid now).
VSL_API int vsl_frame_inputs(void* cmdList, const VslFrame* f) {
    if (!g_frame) return -1;
    float aspect = f->renderHeight ? (float)f->renderWidth / (float)f->renderHeight : 16.0f / 9.0f;
    bool inverted = f->depthInverted != 0;
    sl::Constants c{};
    c.cameraViewToClip = perspective(f->fovY, aspect, f->nearZ, f->farZ, inverted);
    // shortcut: no view matrices from the title; motion vectors carry the
    // camera's motion, so the clip-to-previous-clip transform is identity.
    sl::float4x4 view = c.cameraViewToClip;
    c.clipToCameraView = identity();
    {
        // Inverse of the diagonal-ish projection above.
        float x = view.row[0].x, y = view.row[1].y, a = view.row[2].z, b = view.row[3].z;
        sl::float4x4 inv{};
        inv.row[0] = {1.0f / x, 0, 0, 0};
        inv.row[1] = {0, 1.0f / y, 0, 0};
        inv.row[2] = {0, 0, 0, 1.0f / b};
        inv.row[3] = {0, 0, 1, -a / b};
        c.clipToCameraView = inv;
    }
    c.clipToLensClip = identity();
    c.clipToPrevClip = identity();
    c.prevClipToClip = identity();
    c.jitterOffset = {f->jitterX, f->jitterY};
    c.mvecScale = {f->mvecScaleX, f->mvecScaleY};
    c.cameraPinholeOffset = {0, 0};
    c.cameraPos = {0, 0, 0};
    c.cameraUp = {0, 1, 0};
    c.cameraRight = {1, 0, 0};
    c.cameraFwd = {0, 0, 1};
    c.cameraNear = f->nearZ;
    c.cameraFar = f->farZ;
    c.cameraFOV = f->fovY;
    c.cameraAspectRatio = aspect;
    c.depthInverted = inverted ? sl::Boolean::eTrue : sl::Boolean::eFalse;
    c.cameraMotionIncluded = sl::Boolean::eTrue;
    c.motionVectors3D = sl::Boolean::eFalse;
    c.reset = f->reset ? sl::Boolean::eTrue : sl::Boolean::eFalse;
    c.motionVectorsJittered = sl::Boolean::eFalse;
    sl::Result r = slSetConstants(c, *g_frame, g_viewport);
    if (r != sl::Result::eOk) logf("slSetConstants -> %d", (int)r);

    sl::Extent render{0, 0, f->renderWidth, f->renderHeight};
    sl::Extent output{0, 0, f->outputWidth, f->outputHeight};
    sl::Resource depth{sl::ResourceType::eTex2d, f->depth, nullptr, nullptr, f->depthState};
    sl::Resource mvec{sl::ResourceType::eTex2d, f->mvec, nullptr, nullptr, f->mvecState};
    sl::ResourceTag tags[] = {
        sl::ResourceTag{&depth, sl::kBufferTypeDepth, sl::ResourceLifecycle::eOnlyValidNow, &render},
        sl::ResourceTag{&mvec, sl::kBufferTypeMotionVectors, sl::ResourceLifecycle::eOnlyValidNow, &render},
        sl::ResourceTag{nullptr, sl::kBufferTypeBackbuffer, sl::ResourceLifecycle{}, &output},
    };
    sl::Result t = slSetTagForFrame(*g_frame, g_viewport, tags, 3,
                                    reinterpret_cast<sl::CommandBuffer*>(cmdList));
    if (t != sl::Result::eOk) logf("slSetTagForFrame -> %d", (int)t);
    return (int)(r != sl::Result::eOk ? r : t);
}

VSL_API void vsl_present_start() {
    if (!g_frame) return;
    slPCLSetMarker(sl::PCLMarker::eRenderSubmitEnd, *g_frame);
    slPCLSetMarker(sl::PCLMarker::ePresentStart, *g_frame);
}

VSL_API void vsl_present_end() {
    if (!g_frame) return;
    slPCLSetMarker(sl::PCLMarker::ePresentEnd, *g_frame);
}

VSL_API int vsl_shutdown() {
    sl::Result r = slShutdown();
    logf("slShutdown -> %d", (int)r);
    if (g_log) fclose(g_log);
    g_log = nullptr;
    return (int)r;
}
