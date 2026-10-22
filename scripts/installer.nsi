Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

Name "QuickTool Rust"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\QuickTool Rust"
InstallDirRegKey HKCU "Software\QuickToolRust" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
ShowInstDetails show
ShowUninstDetails show
Icon "${PAYLOAD}\assets\assets\quicktool.ico"
UninstallIcon "${PAYLOAD}\assets\assets\quicktool.ico"
VIProductVersion "0.6.0.0"
VIAddVersionKey /LANG=1033 "ProductName" "QuickTool Rust"
VIAddVersionKey /LANG=1033 "FileDescription" "QuickTool Rust Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "QuickTool contributors"

Var NoIntegration
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\QuickTool.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/NOINTEGRATION" $R1
  ${IfNot} ${Errors}
    StrCpy $NoIntegration "1"
  ${EndIf}
FunctionEnd

Function un.onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/NOINTEGRATION" $R1
  ${IfNot} ${Errors}
    StrCpy $NoIntegration "1"
  ${EndIf}
FunctionEnd

Section "QuickTool Rust" MainSection
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD}\*.*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  ${If} $NoIntegration != "1"
    WriteRegStr HKCU "Software\QuickToolRust" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "DisplayName" "QuickTool Rust"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "DisplayIcon" "$INSTDIR\QuickTool.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust" "NoRepair" 1
    CreateShortcut "$SMPROGRAMS\QuickTool Rust.lnk" "$INSTDIR\QuickTool.exe"
    CreateShortcut "$DESKTOP\QuickTool Rust.lnk" "$INSTDIR\QuickTool.exe"
  ${EndIf}
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  ; Delete only files shipped by this version. Never recursively remove a
  ; user-selected install directory, settings, history, or unrelated files.
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ${If} $NoIntegration != "1"
    Delete "$SMPROGRAMS\QuickTool Rust.lnk"
    Delete "$DESKTOP\QuickTool Rust.lnk"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust"
    DeleteRegKey HKCU "Software\QuickToolRust"
  ${EndIf}
SectionEnd
