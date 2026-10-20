; design:os studio — NSIS Modern UI 2 Per-User Installer
; Builds dist/designos-<version>-<target>-setup.exe via:
;   makensis -DVERSION=<ver> -DOUTFILE=<path> installer.nsi

!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.2.0"
!endif

!ifndef OUTFILE
  !define OUTFILE "designos-${VERSION}-setup.exe"
!endif

Name "design:os studio ${VERSION}"
OutFile "${OUTFILE}"
Unicode True

; Per-user installation — no UAC admin elevation required
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\designos"
InstallDirRegKey HKCU "Software\designos" "Install_Dir"

!define MUI_ABORTWARNING
!if /FileExists "designos.ico"
  !define MUI_ICON "designos.ico"
  !define MUI_UNICON "designos.ico"
!endif

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\designos-studio.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch design:os studio"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "design:os studio (Core & CLI)" SecCore
  SectionIn RO

  SetOutPath "$INSTDIR"
  File "designos-studio.exe"
  File "designos.exe"
  File "designos-mcp.exe"
  File "designos-diff.exe"
  File /nonfatal "designos.ico"
  File /nonfatal "README.md"
  File /nonfatal "LICENSE"
  File /nonfatal "install.ps1"
  File /nonfatal "uninstall.ps1"

  SetOutPath "$INSTDIR\licenses"
  File /nonfatal "licenses\*"

  ; Store installation folder
  WriteRegStr HKCU "Software\designos" "Install_Dir" "$INSTDIR"
  WriteRegStr HKCU "Software\designos" "Version" "${VERSION}"

  ; Write uninstaller
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Start Menu & Desktop shortcuts (colon is reserved in NTFS filenames, so use designos studio.lnk)
  CreateDirectory "$SMPROGRAMS\designos"
  CreateShortCut "$SMPROGRAMS\designos\designos studio.lnk" "$INSTDIR\designos-studio.exe" "" "$INSTDIR\designos-studio.exe" 0
  CreateShortCut "$SMPROGRAMS\designos\Uninstall designos.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortCut "$DESKTOP\designos studio.lnk" "$INSTDIR\designos-studio.exe" "" "$INSTDIR\designos-studio.exe" 0

  ; Register .designos and .fig file associations under HKCU\Software\Classes
  WriteRegStr HKCU "Software\Classes\.designos" "" "designos.Document"
  WriteRegStr HKCU "Software\Classes\designos.Document" "" "design:os studio Document"
  WriteRegStr HKCU "Software\Classes\designos.Document\DefaultIcon" "" '"$INSTDIR\designos-studio.exe",0'
  WriteRegStr HKCU "Software\Classes\designos.Document\shell\open\command" "" '"$INSTDIR\designos-studio.exe" "%1"'

  WriteRegStr HKCU "Software\Classes\.fig" "" "designos.FigmaDocument"
  WriteRegStr HKCU "Software\Classes\designos.FigmaDocument" "" "Figma Design File (design:os studio)"
  WriteRegStr HKCU "Software\Classes\designos.FigmaDocument\DefaultIcon" "" '"$INSTDIR\designos-studio.exe",0'
  WriteRegStr HKCU "Software\Classes\designos.FigmaDocument\shell\open\command" "" '"$INSTDIR\designos-studio.exe" "%1"'

  ; Add/Remove Programs entry (HKCU)
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "DisplayName" "design:os studio"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "Publisher" "design:os studio & designos contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "DisplayIcon" "$INSTDIR\designos-studio.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\designos-studio.exe"
  Delete "$INSTDIR\designos.exe"
  Delete "$INSTDIR\designos-mcp.exe"
  Delete "$INSTDIR\designos-diff.exe"
  Delete "$INSTDIR\designos.ico"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\install.ps1"
  Delete "$INSTDIR\uninstall.ps1"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\licenses"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\designos\designos studio.lnk"
  Delete "$SMPROGRAMS\designos\Uninstall designos.lnk"
  RMDir "$SMPROGRAMS\designos"
  Delete "$DESKTOP\designos studio.lnk"

  DeleteRegKey HKCU "Software\Classes\.designos"
  DeleteRegKey HKCU "Software\Classes\designos.Document"
  DeleteRegKey HKCU "Software\Classes\.fig"
  DeleteRegKey HKCU "Software\Classes\designos.FigmaDocument"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\designos"
  DeleteRegKey HKCU "Software\designos"
SectionEnd
