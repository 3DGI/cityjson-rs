@echo off
setlocal
set "PATH=C:\msys64\mingw64\bin;C:\mingw64\bin;C:\msys64\usr\bin;%PATH%"
set "REAL_SQLITE=C:\msys64\mingw64\bin\sqlite3.exe"
if not exist "%REAL_SQLITE%" set "REAL_SQLITE=C:\mingw64\bin\sqlite3.exe"
if not exist "%REAL_SQLITE%" set "REAL_SQLITE=sqlite3.exe"
call "%REAL_SQLITE%" %*
exit /b %ERRORLEVEL%
