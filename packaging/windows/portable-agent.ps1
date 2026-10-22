# Build outputs and official Codex vendor tree -> a portable Windows package.
param(
    [Parameter(Mandatory = $true)] [string] $BinaryDirectory,
    [Parameter(Mandatory = $true)] [string] $CodexVendorDirectory,
    [Parameter(Mandatory = $true)] [string] $OutputDirectory
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Package = Join-Path $OutputDirectory 'CADCraft-Portatil-v5'
if (Test-Path $Package) { throw "Output already exists: $Package" }
New-Item -ItemType Directory -Force $Package | Out-Null
foreach ($Name in @('cadcraft.exe', 'cadcraft-cli.exe')) {
    $Path = Join-Path $BinaryDirectory $Name
    $Bytes = [IO.File]::ReadAllBytes($Path)
    $Offset = [BitConverter]::ToInt32($Bytes, 0x3c)
    if ([BitConverter]::ToUInt16($Bytes, $Offset + 4) -ne 0x8664) { throw "$Name is not Windows x64" }
    $Expected = if ($Name -eq 'cadcraft.exe') { 2 } else { 3 }
    if ([BitConverter]::ToUInt16($Bytes, $Offset + 0x5c) -ne $Expected) { throw "Wrong PE subsystem: $Name" }
    Copy-Item -LiteralPath $Path -Destination $Package
}
$Codex = Join-Path $Package 'codex'
New-Item -ItemType Directory -Force $Codex | Out-Null
foreach ($Name in @('bin', 'codex-path', 'codex-package.json')) {
    Copy-Item -LiteralPath (Join-Path $CodexVendorDirectory $Name) -Destination $Codex -Recurse
}
$Resources = Join-Path $Codex 'codex-resources'
New-Item -ItemType Directory -Force $Resources | Out-Null
Get-ChildItem -LiteralPath (Join-Path $CodexVendorDirectory 'codex-resources') -File | Copy-Item -Destination $Resources
foreach ($Name in @('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md', 'PDF-NOTICES.txt')) {
    Copy-Item -LiteralPath (Join-Path $Root $Name) -Destination $Package
}
Copy-Item -LiteralPath (Join-Path $Root 'docs/stl-models.md') -Destination (Join-Path $Package 'STL-3D.md')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'portable-launch.cmd') -Destination (Join-Path $Package 'ABRIR-CADCRAFT.cmd')
@'
CADCraft com assistente Codex - Windows x64, versao de teste

Extraia TODO o ZIP para uma pasta nova e abra ABRIR-CADCRAFT.cmd.
No app: Janela > Assistente Codex > Conectar ao Codex > Entrar com o ChatGPT.
Em ingles: Window > Codex Assistant.
MODELOS STL 3D:
Arquivo > Abrir modelo STL, ou arraste seu arquivo .stl sobre o aplicativo.
Arraste com o botao esquerdo para girar a vista, direito para mover, roda para zoom.
Defina a unidade original do STL antes de pedir medidas em mm/cm.
Use os controles para girar, mover, escalar, redimensionar e centralizar.
Salvar STL como exporta o modelo ajustado. Desfazer/Refazer funcionam normalmente.
O agente tambem pode executar esses ajustes e escolher vistas.
Depois de trocar de documento, inicie uma Nova conversa no Codex.
Limites STL: 64 MiB, ate 1.000.000 de triangulos, coordenadas finitas ate +/-1e9.
Edicao do modelo inteiro; sem escultura, geracao de personagens, reparo de malhas,
texturas ou importacao OBJ/STEP/IGES nesta versao. Modelos nao incluidos.

ANEXOS DE REFERENCIA:
Para anexar: Anexar arquivos, ou arraste PDF/PNG/JPG/TXT com o painel aberto.
Escreva o pedido e clique em Enviar. Arquivos so sao enviados nessa etapa.
Limites: 6 arquivos, 20 MiB por arquivo, 40 MiB no total, PDF ate 12 paginas,
ate 16 imagens/paginas por mensagem, TXT ate 160 mil bytes de texto por arquivo.
PDFs sao convertidos em imagens de paginas; anexar nao importa geometria CAD.
Use outro app para DOCX/XLSX, salvando como PDF ou TXT antes de anexar.
Nao precisa instalar Rust, Visual Studio ou Node.js no computador de destino.
Mantenha a pasta codex ao lado de cadcraft.exe. Recursos de voz nao incluidos.
O login real e feito por voce no navegador, pelo Codex oficial.
Esta e uma compilacao de desenvolvimento, nao uma release oficial do projeto.
'@ | Set-Content -LiteralPath (Join-Path $Package 'LEIA-ME.txt') -Encoding utf8
$Licenses = @{
    'LICENSE' = 'https://raw.githubusercontent.com/openai/codex/rust-v0.163.0/LICENSE'
    'NOTICE' = 'https://raw.githubusercontent.com/openai/codex/rust-v0.163.0/NOTICE'
    'RIPGREP-COPYING' = 'https://raw.githubusercontent.com/BurntSushi/ripgrep/14.1.1/COPYING'
    'RIPGREP-LICENSE-MIT' = 'https://raw.githubusercontent.com/BurntSushi/ripgrep/14.1.1/LICENSE-MIT'
    'RIPGREP-UNLICENSE' = 'https://raw.githubusercontent.com/BurntSushi/ripgrep/14.1.1/UNLICENSE'
}
foreach ($Name in $Licenses.Keys) { Invoke-WebRequest -Uri $Licenses[$Name] -OutFile (Join-Path $Codex $Name) }
$Zip = Join-Path $OutputDirectory 'CADCraft-Windows-portatil-v5.zip'
[IO.Compression.ZipFile]::CreateFromDirectory($Package, $Zip)
$Hash = (Get-FileHash -LiteralPath $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
"$Hash  CADCraft-Windows-portatil-v5.zip" | Set-Content -LiteralPath ($Zip + '.sha256') -Encoding ascii
Write-Host "Portable package: $Zip"
