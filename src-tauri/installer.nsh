!macro customUnInstall
  DetailPrint "Cleaning up runtime sidecars..."
  ; Legacy location from the old Python OCR sidecar.
  RMDir /r "$APPDATA\com.pursue-data-analyzer\python-runtime"
  ; Current vision runtime (Python venv + Torch, multi-GB, fully rebuildable).
  RMDir /r "$APPDATA\com.pursue-data-analyzer\vision-runtime"
  ; Rebuildable scratch data; never touches evidence (library/, snapshots/, pursue.db).
  RMDir /r "$APPDATA\com.pursue-data-analyzer\download-parts"
  RMDir /r "$APPDATA\com.pursue-data-analyzer\decrypted-cache"
!macroend
