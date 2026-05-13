@echo off
setlocal
set "PATH=__MINGW_BIN__;__USR_BIN__;%PATH%"
set "REAL_CMAKE=__CMAKE_EXE__"

if not exist "%REAL_CMAKE%" (
  echo Missing MSYS2 CMake at "%REAL_CMAKE%" 1>&2
  exit /b 1
)

if "%~1"=="--build" goto passthrough
if "%~1"=="--install" goto passthrough
if "%~1"=="--version" goto passthrough
if "%~1"=="-E" goto passthrough

call "%REAL_CMAKE%" ^
  -DEXE_SQLITE3=__SQLITE_EXE__ ^
  -DTIFF_INCLUDE_DIR=__TIFF_INCLUDE_DIR__ ^
  -DTIFF_LIBRARY=__TIFF_LIBRARY__ ^
  -DCMAKE_MAKE_PROGRAM=__NINJA_EXE__ ^
  %*
exit /b %ERRORLEVEL%

:passthrough
call "%REAL_CMAKE%" %*
exit /b %ERRORLEVEL%
