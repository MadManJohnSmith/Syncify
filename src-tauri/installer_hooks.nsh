!macro NSIS_HOOK_PREINSTALL
  ; Cerrar instancias previas si estuvieran corriendo antes de actualizar
  nsExec::Exec 'taskkill /F /IM Syncify.exe /T'
  nsExec::Exec 'taskkill /F /IM syncify-tauri.exe /T'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Cerrar Syncify para evitar archivos bloqueados durante la desinstalacion
  nsExec::Exec 'taskkill /F /IM Syncify.exe /T'
  nsExec::Exec 'taskkill /F /IM syncify-tauri.exe /T'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Preguntar al usuario si desea borrar tambien los datos locales y cache
  MessageBox MB_YESNO|MB_ICONQUESTION "¿Deseas eliminar tambien la base de datos local, cuentas y configuracion de Syncify?$\r$\n$\r$\n(Nota: Tus archivos de musica descargados NO seran eliminados)" IDNO skip_cleanup
    RMDir /r "$LOCALAPPDATA\com.syncify.app"
  skip_cleanup:
!macroend
