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
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\QuickTool.exe"
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

; Optional tool modules live in one tree group so the components page offers a
; single select-all/none checkbox over every module. The core section always
; extracts the package store (assets\packages\<id>); each module section copies
; its package into assets\modules\<id>, which the app treats as "installed".
; Modules can also be installed or uninstalled later in the app settings.
Section "QuickTool Rust（核心 / Core）" MainSection
  SectionIn RO
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

SectionGroup /e "功能模块 Modules" SecGroupModules
Section "计算器 Calculator" SecCalculator
  Call CopyModuleCalculator
SectionEnd
Section "时间日期 Time" SecTime
  Call CopyModuleTime
SectionEnd
Section "颜色转换 Color" SecColor
  Call CopyModuleColor
SectionEnd
Section "单位换算 Units" SecUnit
  Call CopyModuleUnit
SectionEnd
Section "进制转换 Radix" SecBase
  Call CopyModuleBase
SectionEnd
Section "JSON 工具" SecJson
  Call CopyModuleJson
SectionEnd
Section "文本与编码 Text" SecText
  Call CopyModuleText
SectionEnd
Section "字符参考 Unicode（约 7 MB）" SecUnicode
  Call CopyModuleUnicode
SectionEnd
Section "国家地区 Country（约 5 MB）" SecCountry
  Call CopyModuleCountry
SectionEnd
Section "钉贴 Pins" SecPin
  Call CopyModulePin
SectionEnd
Section "剪贴板 Clipboard" SecClipboard
  Call CopyModuleClipboard
SectionEnd
Section "文件搜索 Files" SecFiles
  Call CopyModuleFiles
SectionEnd
Section "系统工具 System" SecSystem
  Call CopyModuleSystem
SectionEnd
SectionGroupEnd

; CopyFiles from the package store into the live modules directory; the app
; scans assets\modules\<id>\manifest.json to decide a module is installed.
Function CopyModuleCalculator
  CreateDirectory "$INSTDIR\assets\modules\calculator"
  CopyFiles /SILENT "$INSTDIR\assets\packages\calculator\*.*" "$INSTDIR\assets\modules\calculator"
FunctionEnd
Function CopyModuleTime
  CreateDirectory "$INSTDIR\assets\modules\time"
  CopyFiles /SILENT "$INSTDIR\assets\packages\time\*.*" "$INSTDIR\assets\modules\time"
FunctionEnd
Function CopyModuleColor
  CreateDirectory "$INSTDIR\assets\modules\color"
  CopyFiles /SILENT "$INSTDIR\assets\packages\color\*.*" "$INSTDIR\assets\modules\color"
FunctionEnd
Function CopyModuleUnit
  CreateDirectory "$INSTDIR\assets\modules\unit"
  CopyFiles /SILENT "$INSTDIR\assets\packages\unit\*.*" "$INSTDIR\assets\modules\unit"
FunctionEnd
Function CopyModuleBase
  CreateDirectory "$INSTDIR\assets\modules\base"
  CopyFiles /SILENT "$INSTDIR\assets\packages\base\*.*" "$INSTDIR\assets\modules\base"
FunctionEnd
Function CopyModuleJson
  CreateDirectory "$INSTDIR\assets\modules\json"
  CopyFiles /SILENT "$INSTDIR\assets\packages\json\*.*" "$INSTDIR\assets\modules\json"
FunctionEnd
Function CopyModuleText
  CreateDirectory "$INSTDIR\assets\modules\text"
  CopyFiles /SILENT "$INSTDIR\assets\packages\text\*.*" "$INSTDIR\assets\modules\text"
FunctionEnd
Function CopyModuleUnicode
  CreateDirectory "$INSTDIR\assets\modules\unicode"
  CopyFiles /SILENT "$INSTDIR\assets\packages\unicode\*.*" "$INSTDIR\assets\modules\unicode"
FunctionEnd
Function CopyModuleCountry
  CreateDirectory "$INSTDIR\assets\modules\country"
  CopyFiles /SILENT "$INSTDIR\assets\packages\country\*.*" "$INSTDIR\assets\modules\country"
FunctionEnd
Function CopyModulePin
  CreateDirectory "$INSTDIR\assets\modules\pin"
  CopyFiles /SILENT "$INSTDIR\assets\packages\pin\*.*" "$INSTDIR\assets\modules\pin"
FunctionEnd
Function CopyModuleClipboard
  CreateDirectory "$INSTDIR\assets\modules\clipboard"
  CopyFiles /SILENT "$INSTDIR\assets\packages\clipboard\*.*" "$INSTDIR\assets\modules\clipboard"
FunctionEnd
Function CopyModuleFiles
  CreateDirectory "$INSTDIR\assets\modules\files"
  CopyFiles /SILENT "$INSTDIR\assets\packages\files\*.*" "$INSTDIR\assets\modules\files"
FunctionEnd
Function CopyModuleSystem
  CreateDirectory "$INSTDIR\assets\modules\system"
  CopyFiles /SILENT "$INSTDIR\assets\packages\system\*.*" "$INSTDIR\assets\modules\system"
FunctionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${MainSection} "QuickTool 主程序。模块安装包始终落盘，可随时在应用设置 → 模块管理中安装或卸载。Core program; module packages always ship so modules can be installed or removed later in the app settings."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecGroupModules} "可选功能模块；勾选组节点即可全选或全不选。Optional modules; tick the group node to select or clear all."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCalculator} "表达式、函数与常量求值。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecTime} "日期、Unix 时间戳与其他时间格式互转。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecColor} "颜色预览、取色与多种色值格式互转。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnit} "长度、质量、面积等常用单位换算。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecBase} "二、八、十、十六进制互转。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecJson} "JSON 格式化、压缩与检查。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecText} "大小写、Base64、URL、转义、全半角、文本处理、UUID 与随机串。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnicode} "Unicode 字符、数学符号、希腊字母、注音与 Emoji 查询。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCountry} "国家/地区代码、区号查询与国旗。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecPin} "把文字、图片和文件钉在桌面上。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecClipboard} "查看剪贴板原始数据与格式。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecFiles} "本地文件与目录名索引搜索。"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecSystem} "系统命令、Windows 设置搜索与注册表查询。"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  SetShellVarContext current
  ; Delete only files shipped by this version. Never recursively remove a
  ; user-selected install directory, settings, history, or unrelated files.
  !include "${UNINSTALL_FILES}"
  ; Module instances appear at runtime (component page and app settings), so
  ; remove that one directory explicitly.
  RMDir /r "$INSTDIR\assets\modules"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ${If} $NoIntegration != "1"
    Delete "$SMPROGRAMS\QuickTool Rust.lnk"
    Delete "$DESKTOP\QuickTool Rust.lnk"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\QuickToolRust"
    DeleteRegKey HKCU "Software\QuickToolRust"
  ${EndIf}
SectionEnd
