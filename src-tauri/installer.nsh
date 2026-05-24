!macro customUnInstall
  DetailPrint "Cleaning up python-runtime sidecar..."
  RMDir /r "$APPDATA\com.pursue-data-analyzer\python-runtime"
  
  DetailPrint "Cleaning up OSINT evidence databases..."
  ; Note: We generally don't wipe user data unless explicitly requested, 
  ; but this hook shows how we can wipe the runtime to prevent bloat.
!macroend
