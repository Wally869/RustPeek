@echo off
setlocal

set RUSTPEEK=target\debug\rustpeek.exe

echo rustpeek check - testing all samples
echo.

for /d %%D in (samples\*) do (
    echo %%~nxD
    %RUSTPEEK% check "%%D"
    echo.
)

endlocal
