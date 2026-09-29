@echo off
title Charybdis Mini RMK Flasher
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0flash.ps1" %*
pause
