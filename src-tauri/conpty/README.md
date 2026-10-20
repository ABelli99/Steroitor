# ConPTY

`conpty.dll` e `x64/OpenConsole.exe` dal pacchetto NuGet
[Microsoft.Windows.Console.ConPTY](https://www.nuget.org/packages/Microsoft.Windows.Console.ConPTY)
1.25.260930003 (Microsoft, licenza MIT, firmati Authenticode da Microsoft).

Finiscono nella cartella di installazione, accanto a `steroitor.exe`: portable-pty carica `conpty.dll`
da lì al posto della ConPTY di sistema. Quella di Windows 10 scarta il testo attenuato (`ESC[2m`) e
ridisegna male le TUI come Claude Code.

Per aggiornarli: scaricare il `.nupkg`, prendere `runtimes/win-x64/native/conpty.dll` e
`build/native/runtimes/x64/OpenConsole.exe`, verificare la firma con `Get-AuthenticodeSignature`.
