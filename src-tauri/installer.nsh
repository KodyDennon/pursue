; Tauri v2 installer hooks. Updates and ordinary uninstall/reinstall operations
; must never remove the evidence vault, database, model cache, custom-storage
; pointer, resumable downloads, or exports. Destructive removal is available
; only through the explicit Factory Reset flow inside the application.
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Preserving PURSUE user data and selected acceleration lane..."
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Checking the signed Microsoft Visual C++ runtime prerequisite..."
  IfFileExists "$INSTDIR\assets\native_runtime\vc_redist.x64.exe" 0 pursue_vcredist_missing
  ExecWait '"$INSTDIR\assets\native_runtime\vc_redist.x64.exe" /install /quiet /norestart' $0
  IntCmp $0 0 pursue_vcredist_done
  IntCmp $0 3010 pursue_vcredist_done
  IntCmp $0 1638 pursue_vcredist_done
  MessageBox MB_ICONSTOP "Microsoft Visual C++ Runtime installation failed with exit code $0. PURSUE was not modified and your data remains intact."
  Abort
  pursue_vcredist_missing:
  MessageBox MB_ICONSTOP "The signed Microsoft Visual C++ Runtime prerequisite is missing from this installer. PURSUE cannot be installed safely."
  Abort
  pursue_vcredist_done:
  DetailPrint "PURSUE application files installed; persistent data was left unchanged."
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing application files only; PURSUE user data will be preserved."
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "PURSUE user data remains available for reinstall or manual recovery."
!macroend
