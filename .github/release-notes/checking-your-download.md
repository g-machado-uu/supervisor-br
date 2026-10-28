### Conferindo o download (opcional)

O `SHA256SUMS.txt` traz um código (uma espécie de impressão digital) para cada programa. Se o programa que você baixou der o mesmo código, ele é exatamente o arquivo que foi compilado. Descompacte o download primeiro: os códigos são do programa dentro do zip, não do zip.

**Windows**
1. Abra o **PowerShell** pelo menu Iniciar.
2. Digite `Get-FileHash ` (com um espaço no final), arraste o `supervisor-br.exe` para a janela e aperte Enter.
3. Dê um clique duplo no código longo abaixo de **Hash** para selecioná-lo e copie com Ctrl+C.
4. Abra o `SHA256SUMS.txt` no Bloco de Notas, aperte Ctrl+F e cole. Ele deve ser encontrado na linha que termina em `supervisor-br-Windows/supervisor-br.exe`.

**macOS**
1. Abra o **Terminal** (Aplicativos › Utilitários).
2. Digite `shasum -a 256 ` (com um espaço no final), arraste o arquivo `supervisor-br` descompactado para a janela e aperte Enter.
3. Dê um clique duplo no código longo para selecioná-lo e copie com Cmd+C.
4. Abra o `SHA256SUMS.txt` no TextEdit, aperte Cmd+F e cole. Ele deve ser encontrado na linha que termina em `supervisor-br-macOS/supervisor-br`.

**Linux**
Na pasta com o zip e o `SHA256SUMS.txt`, rode:

    unzip supervisor-br-Linux.zip -d supervisor-br-Linux
    sha256sum -c --ignore-missing SHA256SUMS.txt

Deve aparecer `supervisor-br-Linux/supervisor-br: OK`.

Se o código não for encontrado, ou se o Linux mostrar `FAILED`, não execute o programa. Baixe de novo e, se ainda assim não conferir, entre em contato.
