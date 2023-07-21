import * as vscode from "vscode";
import { ctx } from "../spIndex";
import * as fs from "fs";
import { execFile } from "child_process";

export async function run(args: any) {
  const panel = vscode.window.createWebviewPanel(
    "sourcepawnDoctor",
    "SourcePawn Doctor",
    vscode.ViewColumn.One,
    {}
  );

  const doctor = new Doctor();
  doctor.runDiagnostics();

  const updateWebview = () => {
    panel.webview.html = doctor.toWebview();
  };

  // Set initial content
  updateWebview();

  // And schedule updates to the content every second
  setInterval(updateWebview, 100);
}

enum DiagnosticState {
  OK,
  Warning,
  Error,
  None,
}

class Doctor {
  // Language server
  lspVersion: string | undefined = undefined;
  isLSPInstalled = DiagnosticState.None;
  isLSPExecutable = DiagnosticState.None;

  // Settings
  spCompPath: string | undefined = undefined;
  isSPCompSet = DiagnosticState.None;
  isSPCompInstalled = DiagnosticState.None;
  isSPCompRunnable = DiagnosticState.None;

  isSMInstalled = DiagnosticState.None;
  isMainPathSet = DiagnosticState.None;
  isMainPathValid = DiagnosticState.None;
  isMainPathCorrect = DiagnosticState.None;

  constructor() {}

  toWebview(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Cat Coding</title>
</head>
<body>
    <h1>SourcePawn Doctor</h1>
    <h2>Language Server</h2>
    <ul>
      ${this.lspDiagnosticToWebview()}
    </ul>
    <h2>Compiler (spcomp)</h2>
    <ul>
      ${this.spCompToWebView()}
    </ul>
    <h2>Includes</h2>
    <ul>
      ${this.includesDirectoriesToWebView()}
    </ul>
    <h2>Main Path</h2>
    <ul>
      ${this.mainPathToWebView()}
    </ul>

    <h2>Additional help</h2>
    <p>If all the above are green and the extension is still not behaving as expected, try the following:</p>
    <ul>
      <li>Restart the SourcePawn Language Server (Hover your mouse on the "sourcepawn-lsp" logo on the bottom left of the screen and click on restart).</li>
      <li>Reload VSCode (CTRL+Shift+P and type "Reload Window").</li>
      <li>Reinstall the SourcePawn Language Server (CTRL+Shift+P and type "Install Sourcepawn Language Server").</li>
      <li>Look in the logs for errors (Hover your mouse on the "sourcepawn-lsp" logo on the bottom left of the screen and click on Open Logs). You can set the verbosity of the server to "trace" in the "sourcepawn.trace.server" setting.</li>
      <li>Try to reproduce the issue in a new project.</li>
      <li>If the extension is still not working properly, try contacting Sarrus on Discord (sarrus_).</li>
      </ul>
</body>
</html>`;
  }

  async runDiagnostics() {
    this.checkLSP();
    this.checkSettings();
    this.checkIncludesDirectories();
    this.checkSpComp();
  }

  lspDiagnosticToWebview(): string {
    const diagnostics = [];
    switch (this.isLSPInstalled) {
      case DiagnosticState.OK:
        diagnostics.push("✅ Language Server installed.");
        break;
      case DiagnosticState.Error:
        diagnostics.push("❌ Language Server not installed.");
        break;
      case DiagnosticState.None:
        diagnostics.push("🩺 Checking if the Language Server is installed.");
        break;
    }
    switch (this.isLSPExecutable) {
      case DiagnosticState.OK:
        diagnostics.push("✅ Language Server is executable.");
        break;
      case DiagnosticState.Error:
        diagnostics.push("❌ Language Server is not executable.");
        break;
      case DiagnosticState.None:
        diagnostics.push("🩺 Checking if the Language Server is executable.");
        break;
    }

    return diagnostics.map((d) => `<li>${d}</li>`).join("\n");
  }

  async checkLSP() {
    fs.stat(ctx?.serverPath, (err, _stats) => {
      if (err) {
        this.isLSPInstalled = DiagnosticState.Error;
        return;
      }
      if (!_stats?.isFile()) {
        this.isLSPInstalled = DiagnosticState.Error;
        return;
      }
      this.isLSPInstalled = DiagnosticState.OK;
    });
    const version = await ctx?.getServerVersionFromBinaryAsync();
    if (version === undefined) {
      this.isLSPExecutable = DiagnosticState.Error;
      return;
    }
    this.isLSPExecutable = DiagnosticState.OK;
    this.lspVersion = version;
  }

  spCompToWebView(): string {
    const diagnostics = [];
    switch (this.isSPCompSet) {
      case DiagnosticState.OK:
        diagnostics.push(
          `✅ "SourcePawnLanguageServer.spcompPath" is set (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          '❌ "SourcePawnLanguageServer.spcompPath" is empty. You should set it to the path of the "spcomp" executable.'
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.spcompPath" is set.'
        );
        break;
    }

    switch (this.isSPCompInstalled) {
      case DiagnosticState.OK:
        diagnostics.push(
          `✅ "SourcePawnLanguageServer.spcompPath" points to a file (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          `❌ "SourcePawnLanguageServer.spcompPath" does not point to a file (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.spcompPath" points to a file.'
        );
        break;
    }

    switch (this.isSPCompRunnable) {
      case DiagnosticState.OK:
        diagnostics.push(
          `✅ "SourcePawnLanguageServer.spcompPath" is executable (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          `❌ "SourcePawnLanguageServer.spcompPath" is not executable (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.spcompPath" is executable.'
        );
        break;
    }

    return diagnostics.map((d) => `<li>${d}</li>`).join("\n");
  }

  async checkSpComp() {
    this.spCompPath = vscode.workspace
      .getConfiguration("SourcePawnLanguageServer")
      .get("spcompPath");
    if (!this.spCompPath) {
      this.isSPCompSet = DiagnosticState.Error;
      this.isSPCompInstalled = DiagnosticState.Error;
      this.isSPCompRunnable = DiagnosticState.Error;
      return;
    }
    this.isSPCompSet = DiagnosticState.OK;
    fs.stat(this.spCompPath, (err, _stats) => {
      if (err) {
        this.isSPCompInstalled = DiagnosticState.Error;
        this.isSPCompRunnable = DiagnosticState.Error;
        return;
      }
      if (!_stats?.isFile()) {
        this.isSPCompInstalled = DiagnosticState.Error;
        this.isSPCompRunnable = DiagnosticState.Error;
        return;
      }
      this.isSPCompInstalled = DiagnosticState.OK;

      execFile(this.spCompPath, ["-h"], (err, stdout, stderr) => {
        if (err) {
          if (stdout.startsWith("SourcePawn Compiler")) {
            this.isSPCompRunnable = DiagnosticState.OK;
            return;
          }
          this.isSPCompRunnable = DiagnosticState.Error;
          return;
        }
        this.isSPCompRunnable = DiagnosticState.OK;
      });
    });
  }

  includesDirectoriesToWebView(): string {
    const diagnostics = [];
    switch (this.isSMInstalled) {
      case DiagnosticState.OK:
        diagnostics.push(
          '✅ "SourcePawnLanguageServer.includeDirectories" contains at least one entry that contains "sourcemod.inc".'
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          '❌ "SourcePawnLanguageServer.includeDirectories" contains at least one invalid entry".'
        );
        break;
      case DiagnosticState.Warning:
        diagnostics.push(
          '⚠️ "SourcePawnLanguageServer.includeDirectories" contains at least one entry that was not scanned properly.'
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.includeDirectories" is set.'
        );
        break;
    }

    return diagnostics.map((d) => `<li>${d}</li>`).join("\n");
  }

  async checkIncludesDirectories() {
    const includesDirectories: string[] = vscode.workspace
      .getConfiguration("SourcePawnLanguageServer")
      .get("includesDirectories");
    if (!includesDirectories) {
      this.isSMInstalled = DiagnosticState.Error;
      return;
    }
    includesDirectories.forEach((dir) => {
      if (this.isSMInstalled !== DiagnosticState.None) return;
      fs.stat(dir, (err, _stats) => {
        if (err) {
          this.isSMInstalled = DiagnosticState.Warning;
          return;
        }
        if (!_stats?.isDirectory()) {
          this.isSMInstalled = DiagnosticState.Error;
          return;
        }
        fs.readdir(dir, (err, files) => {
          if (err) {
            this.isSMInstalled = DiagnosticState.Error;
            return;
          }
          files.forEach((file) => {
            if (file === "sourcemod.inc") {
              this.isSMInstalled = DiagnosticState.OK;
              return;
            }
          });
        });
      });
    });
  }

  mainPathToWebView(): string {
    const diagnostics = [];
    switch (this.isMainPathSet) {
      case DiagnosticState.OK:
        diagnostics.push('✅ "SourcePawnLanguageServer.mainPath" is set.');
        break;
      case DiagnosticState.Error:
        diagnostics.push('❌ "SourcePawnLanguageServer.mainPath" is not set.');
        break;
      case DiagnosticState.Warning:
        diagnostics.push(
          '⚠️ "SourcePawnLanguageServer.mainPath" is not set. Consider setting it for the extension to work properly.'
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.mainPath" is set.'
        );
        break;
    }

    switch (this.isMainPathValid) {
      case DiagnosticState.OK:
        diagnostics.push(
          `✅ "SourcePawnLanguageServer.mainPath" points to a file (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          `❌ "SourcePawnLanguageServer.mainPath" does not point to a file (value: ${this.spCompPath}).`
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.mainPath" points to a file.'
        );
        break;
    }

    switch (this.isMainPathCorrect) {
      case DiagnosticState.OK:
        diagnostics.push(
          `✅ "SourcePawnLanguageServer.mainPath" points to a file that contains "OnPluginStart".`
        );
        break;
      case DiagnosticState.Warning:
        diagnostics.push(
          `⚠️ "SourcePawnLanguageServer.mainPath" points to a file that does not contain "OnPluginStart". This does not mean that it is incorrect.`
        );
        break;
      case DiagnosticState.Error:
        diagnostics.push(
          `❌ "SourcePawnLanguageServer.mainPath" points to a file of which the content cannot be read`
        );
        break;
      case DiagnosticState.None:
        diagnostics.push(
          '🩺 Checking if "SourcePawnLanguageServer.mainPath" points to a file that contains "OnPluginStart".'
        );
        break;
    }

    return diagnostics.map((d) => `<li>${d}</li>`).join("\n");
  }

  async checkSettings() {
    this.checkSpComp();
    this.checkIncludesDirectories();
    const mainPath: string = vscode.workspace
      .getConfiguration("SourcePawnLanguageServer")
      .get("mainPath");
    if (!mainPath) {
      this.isMainPathSet = DiagnosticState.Warning;
      this.isMainPathValid = DiagnosticState.Warning;
      this.isMainPathCorrect = DiagnosticState.Warning;
      return;
    }
    this.isMainPathSet = DiagnosticState.OK;
    fs.stat(mainPath, (err, _stats) => {
      if (err) {
        this.isMainPathValid = DiagnosticState.Error;
        this.isMainPathCorrect = DiagnosticState.Error;
        return;
      }
      if (!_stats?.isFile()) {
        this.isMainPathValid = DiagnosticState.Error;
        this.isMainPathCorrect = DiagnosticState.Error;
        return;
      }
      this.isMainPathValid = DiagnosticState.OK;
      fs.readFile(mainPath, (err, files) => {
        if (err) {
          this.isMainPathCorrect = DiagnosticState.Error;
          return;
        }
        if (!files.toString().includes("OnPluginStart")) {
          this.isMainPathCorrect = DiagnosticState.Warning;
          return;
        }
        this.isMainPathCorrect = DiagnosticState.OK;
      });
    });
  }
}

export function buildDoctorStatusBar() {
  const doctorStatusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left
  );
  doctorStatusBar.show();
  doctorStatusBar.tooltip = new vscode.MarkdownString(
    "Sourcepawn Doctor helps you diagnose why the extension is not working.",
    true
  );
  doctorStatusBar.tooltip.isTrusted = true;
  doctorStatusBar.text = "$(lightbulb-autofix) Sourcepawn Doctor";
  doctorStatusBar.command = "sourcepawn-vscode.doctor";
}
