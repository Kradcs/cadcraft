# Codex assistant inside CADCraft

The desktop app includes a conversation panel under **Window → Codex Assistant**
(**Janela → Assistente Codex** in Brazilian Portuguese). Ask the assistant to create
or modify geometry in the drawing that is open in CADCraft. For example:

> Desenhe um retângulo de 10 por 6 e um círculo de raio 1 no centro.

## Portable Windows development build

For the portable package, extract the entire ZIP into a new directory and run
`ABRIR-CADCRAFT.cmd`. It sets the Codex executable path to the included official
Windows binary; neither npm nor build tools are needed on the destination PC.
Keep the `codex` directory alongside `cadcraft.exe`. `VERIFICAR.cmd` reports the
versions of both executables if startup needs diagnosis. This is an experimental
cross-compiled build, not an official signed release. Windows execution must be
verified on the destination machine.

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
cargo test --locked -p cadcraft-ui-egui --lib agent::tests
cargo check --locked --target wasm32-unknown-unknown -p cadcraft-ui-egui
```
