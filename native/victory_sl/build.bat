@echo off
rem Builds victory_sl.dll against the Streamline SDK. Usage: build.bat <streamline sdk root> <out dir>
setlocal
set SL=%~1
set OUT=%~2
call "F:\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
if not exist "%OUT%" mkdir "%OUT%"
cl /nologo /LD /EHsc /O2 /std:c++17 /W3 /I "%SL%\include" "%~dp0victory_sl.cpp" /Fo"%OUT%\\" /Fe"%OUT%\victory_sl.dll" /link /LIBPATH:"%SL%\lib\x64" sl.interposer.lib d3d12.lib dxgi.lib || exit /b 1
echo built %OUT%\victory_sl.dll
