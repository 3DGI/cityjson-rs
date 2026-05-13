@echo off
setlocal
set "PATH=C:\msys64\mingw64\bin;C:\mingw64\bin;C:\msys64\usr\bin;%PATH%"
set "REAL_CMAKE=C:\msys64\mingw64\bin\cmake.exe"
if not exist "%REAL_CMAKE%" set "REAL_CMAKE=C:\mingw64\bin\cmake.exe"
if not exist "%REAL_CMAKE%" set "REAL_CMAKE=C:\Program Files\CMake\bin\cmake.exe"

if "%~1"=="--build" goto passthrough
if "%~1"=="--install" goto passthrough
if "%~1"=="--version" goto passthrough
if "%~1"=="-E" goto passthrough

call "%REAL_CMAKE%" ^
  -DEXE_SQLITE3=__SQLITE_CMD__ ^
  -DTIFF_INCLUDE_DIR=C:/msys64/mingw64/include ^
  -DTIFF_LIBRARY=C:/msys64/mingw64/lib/libtiff.dll.a ^
  %*
exit /b %ERRORLEVEL%

:passthrough
call "%REAL_CMAKE%" %*
exit /b %ERRORLEVEL%
