@echo off
setlocal

set RUSTPEEK=target\debug\rustpeek.exe
set TEMP_DIR=samples_fixtest

if exist %TEMP_DIR% rmdir /s /q %TEMP_DIR%
xcopy /e /i /q samples %TEMP_DIR% >nul

echo rustpeek fix test - check, fix, check
echo.

for /d %%D in (%TEMP_DIR%\*) do (
    echo === %%~nxD ===
    echo -- check --
    %RUSTPEEK% check "%%D"
    echo -- fix --
    %RUSTPEEK% fix "%%D"
    echo -- recheck --
    %RUSTPEEK% check "%%D"
    echo.
)

rmdir /s /q %TEMP_DIR%

endlocal
