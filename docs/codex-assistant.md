# Codex assistant inside CADCraft

The desktop app includes a conversation panel under **Window → Codex Assistant**
(**Janela → Assistente Codex** in Brazilian Portuguese). Ask the assistant to create
or modify geometry in the drawing that is open in CADCraft. For example:

> Desenhe um retângulo de 10 por 6 e um círculo de raio 1 no centro.

## Portable Windows development build

For the portable package, extract the entire ZIP into a new directory and run
`ABRIR-CADCRAFT.cmd`. It sets the Codex executable path to the included official
Windows binary; neither npm nor build tools are needed on the destination PC.
Keep the `codex` directory alongside `cadcraft.exe`. The portable ZIP is built and tested on a native Windows GitHub runner. It is an
experimental development build, not an official signed release.

## Windows installation from the source package

Extract the entire source ZIP, then run `INSTALAR.cmd`. It runs
`packaging/windows/install-agent.ps1` to install missing prerequisites through
winget, build the included source with Rust 1.95.0, install it under
`%LOCALAPPDATA%\Programs\CADCraft-Agent`, and create a desktop shortcut.
This is a source installer, not a precompiled Windows release. The Windows
installation itself has not been executed in the Linux development environment.
The launcher selects the native `codex.exe` installed by npm using the
`CADCRAFT_CODEX_EXECUTABLE` process variable, avoiding reliance on npm shell shims.

## Connect your ChatGPT account

1. Install the official [Codex CLI](https://developers.openai.com/codex/cli).
   CADCraft's integration uses the experimental app-server dynamic tool API and
   was developed against Codex CLI 0.163.0. Earlier versions may need updating.
2. Open the panel. Leave the executable as `codex` if it is on `PATH`, or enter
   its full executable path. Choose **Connect to Codex**.
3. If Codex is already signed in, CADCraft reuses that account. Otherwise choose
   **Sign in with ChatGPT**, follow the browser link, and complete the official
   Codex sign-in flow. Account eligibility and usage limits are managed by Codex.
4. Open or create a drawing, enter a request, and choose **Send**.

CADCraft launches `codex app-server` as its own child process and communicates over
stdio JSON lines. It never reads authentication files or stores a ChatGPT password,
API key, or access token in the drawing or interface preferences. Codex manages
its own authentication. The Codex executable uses the current user's Codex
configuration. Requests and drawing inspection results are sent to the configured
Codex model provider. Do not share drawings with that provider unless authorized.

## STL 3D models

Windows portable v5 can open standalone STL meshes in a dedicated 3D workspace.
Use **File → Open STL Model…** or drag an STL onto CADCraft. Codex can inspect and
transform the open model, set/convert units and choose camera views. See
[STL 3D workspace](stl-models.md) for controls, examples and limits. STL documents
are opened locally, separately from conversation reference attachments.

## Attach reference files

Choose **Attach files** (**Anexar arquivos**) in the Codex panel, or drop files
onto the application while the panel is open. You can mix PDF, PNG, JPG/JPEG and
TXT files, review their names and sizes, remove individual files with **×**, or
cancel/clear the selection. Enter your request and choose **Send**. Files are
prepared locally in a background worker; their content is sent to the Codex
provider only when you send the message. Selecting files does not modify the drawing.

Images are decoded, resized to at most 1600 pixels on their longest edge and
sent as PNG image inputs. Every PDF page is rendered to an image, including
embedded scans, text and vector diagrams. PDF references are visual, not a native
CAD import or a searchable text extraction; use TXT for long text and provide
explicit dimensions for precise drafting. Password-protected, damaged or
unsupported PDFs report errors. A PDF rendering warning rejects the file rather
than silently sending an incomplete page. Export such pages as PNG/JPG if needed.

Limits are checked before sending; exceeding one reports an error instead of
silently truncating the attachment:

- 6 files; 20 MiB per source file, 40 MiB source total.
- 1–12 pages per PDF; at most 16 images/PDF pages combined per message.
- PNG/JPEG inputs up to 32 megapixels; 24 MiB total prepared input.
- TXT must use UTF-8 (optional BOM) or UTF-16 with BOM; at most 160,000 decoded
  UTF-8 bytes per file and 240,000 total text bytes per message.

DOCX, XLSX and other formats must first be exported to PDF or TXT. Dropped
DWG/DXF files continue through the normal drawing-opening flow. The pending
selection is an in-memory snapshot: later external changes to the original file
do not alter it. Chat and attachment selections are not saved between app runs.
Content is treated as reference data; drawing tool permissions are unchanged.

## What the agent can do

The model receives three drawing tools: inspect the current drawing, discover
available commands and their parameter documentation, and execute a supported
command. The first version exposes common drawing, editing, layer, selection,
view, dimension and inquiry commands. It uses the actual CADCraft command engine;
it does not generate a script for the user to paste. Command results and errors
are returned to the agent so it can inspect and verify its work.

Each successful edit uses CADCraft's normal undo history. Use **Undo** to revert
an edit. **Stop** disconnects the child process and discards queued agent requests;
completed edits remain in the drawing. Reconnect to continue. Closing the panel
only hides it; use **Stop** or **Disconnect** to end the connection.

A conversation is bound to the drawing's runtime identity. If the active drawing
changes, tool calls are refused. Choose **New conversation** before sending a
request for another drawing. An active interactive drafting command also blocks
agent tool execution until you finish or cancel that command.

The drawing tool boundary rejects file open/save/export, scripts, Lisp, plugins,
process commands, and application-close commands. Saving stays under the user's
control. Shell and freeform patch tools are disabled in the child Codex process;
the assistant is instructed to use only the CADCraft drawing tools. This is not a
replacement for the operating system's permissions or the user's Codex settings.

## Current scope and validation

This is a desktop integration. The WebAssembly build retains the panel's desktop
availability message and does not launch processes. Conversations, panel visibility
and a custom executable path are kept only for the current app session. No persistent
chat history or model picker is provided by CADCraft in this version.

Protocol negotiation and dynamic tool registration can be checked without a model
request. The agent unit tests exercise a simulated Codex conversation through the
real command engine, inspect the created entity, undo it, and verify rejection of
stopped, disallowed, wrong-drawing and interactive-command requests. A signed-in
live request is a separate validation step requiring the user's Codex account.

```sh
cargo test --locked -p cadcraft-ui-egui --lib agent::
cargo check --locked --target wasm32-unknown-unknown -p cadcraft-ui-egui
```

## Measured 3D construction (v6)

The agent can use `model.build`, `model.edit`, `model.add`, `model.remove`, `model.from2d` and `model.to2d`. Inspect `mesh.inspect` for named recipes and whole-model transforms; use `ui.mesh.view` to fit afterward. See [3D modeling](modeling-3d.md) for units, examples, faceted geometry limits and export scope.
