import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";

let diagnosticCollection: vscode.DiagnosticCollection;

export function activate(context: vscode.ExtensionContext) {
  diagnosticCollection =
    vscode.languages.createDiagnosticCollection("flagfile");
  context.subscriptions.push(diagnosticCollection);

  context.subscriptions.push(
    vscode.commands.registerCommand("flagfile.validate", () => {
      const file = getActiveFlagfile();
      if (file) {
        runValidate(file);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("flagfile.lint", () => {
      const file = getActiveFlagfile();
      if (file) {
        runLint(file);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("flagfile.test", () => {
      const file = getActiveFlagfile();
      if (file) {
        runTests(file);
      }
    })
  );

  // Run on save
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (!isRunnableFlagfile(doc)) {
        return;
      }

      const config = vscode.workspace.getConfiguration("flagfile");

      if (config.get<boolean>("validateOnSave", true)) {
        runValidate(doc.uri);
      }
      if (config.get<boolean>("lintOnSave", true)) {
        runLint(doc.uri);
      }
    })
  );

  // Clear diagnostics when file is closed
  context.subscriptions.push(
    vscode.workspace.onDidCloseTextDocument((doc) => {
      diagnosticCollection.delete(doc.uri);
    })
  );
}

export function deactivate() {
  diagnosticCollection?.dispose();
}

function isFlagfile(doc: vscode.TextDocument): boolean {
  if (doc.languageId === "flagfile") {
    return true;
  }
  const name = path.basename(doc.uri.fsPath);
  return name === "Flagfile" || name.startsWith("Flagfile.");
}

function isRunnableFlagfile(doc: vscode.TextDocument): boolean {
  const name = path.basename(doc.uri.fsPath);
  // .tests files get syntax highlighting but not lint/validate/test
  if (name.endsWith(".tests")) {
    return false;
  }
  return isFlagfile(doc);
}

function getActiveFlagfile(): vscode.Uri | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage("No active editor");
    return undefined;
  }
  if (!isRunnableFlagfile(editor.document)) {
    vscode.window.showWarningMessage("Active file is not a Flagfile");
    return undefined;
  }
  return editor.document.uri;
}

function getFFPath(): string {
  return vscode.workspace
    .getConfiguration("flagfile")
    .get<string>("executablePath", "ff");
}

function getWorkDir(fileUri: vscode.Uri): string {
  const folder = vscode.workspace.getWorkspaceFolder(fileUri);
  return folder ? folder.uri.fsPath : path.dirname(fileUri.fsPath);
}

function runValidate(fileUri: vscode.Uri) {
  const ff = getFFPath();
  const filePath = fileUri.fsPath;
  const cwd = getWorkDir(fileUri);

  execFile(ff, ["validate", "-f", filePath], { cwd }, (err, stdout, stderr) => {
    const diagnostics: vscode.Diagnostic[] = [];

    if (err && err.code !== 0) {
      // Parse error — show at top of file
      const msg = stderr.trim() || stdout.trim() || "Validation failed";
      const diag = new vscode.Diagnostic(
        new vscode.Range(0, 0, 0, 0),
        msg,
        vscode.DiagnosticSeverity.Error
      );
      diag.source = "ff validate";
      diagnostics.push(diag);
    }

    // Merge with existing lint diagnostics
    const existing = diagnosticCollection.get(fileUri) || [];
    const lintDiags = [...existing].filter(
      (d) => d.source === "ff lint"
    );
    diagnosticCollection.set(fileUri, [...diagnostics, ...lintDiags]);

    if (!err) {
      const lines = stdout.trim().split("\n");
      const summary = lines[lines.length - 1];
      vscode.window.setStatusBarMessage(`Flagfile: ${summary}`, 3000);
    }
  });
}

function runLint(fileUri: vscode.Uri) {
  const ff = getFFPath();
  const filePath = fileUri.fsPath;
  const cwd = getWorkDir(fileUri);

  execFile(ff, ["lint", "-f", filePath], { cwd }, (_err, _stdout, stderr) => {
    const diagnostics: vscode.Diagnostic[] = [];
    const doc = vscode.workspace.textDocuments.find(
      (d) => d.uri.fsPath === filePath
    );
    const text = doc?.getText() || "";
    const lines = text.split("\n");

    for (const line of stderr.split("\n")) {
      const trimmed = line.replace(/\x1b\[[0-9;]*m/g, "").trim();
      if (!trimmed.startsWith("\u26a0")) {
        continue;
      }

      // Extract flag name from warning
      const content = trimmed.replace(/^\u26a0\s*/, "");
      const flagMatch = content.match(/^(FF[-_][a-zA-Z0-9_-]+)/);
      if (!flagMatch) {
        continue;
      }

      const flagName = flagMatch[1];
      let lineNum = 0;

      // Find the flag definition line
      for (let i = 0; i < lines.length; i++) {
        const l = lines[i].trimStart();
        if (l.startsWith(flagName + " ") || l.startsWith(flagName + "\t") ||
            l === flagName || l.startsWith(flagName + "{")) {
          // Make sure it's not in a comment
          const commentIdx = lines[i].indexOf("//");
          const flagIdx = lines[i].indexOf(flagName);
          if (commentIdx === -1 || flagIdx < commentIdx) {
            lineNum = i;
            break;
          }
        }
      }

      const severity = content.includes("expired")
        ? vscode.DiagnosticSeverity.Error
        : content.includes("deprecated")
        ? vscode.DiagnosticSeverity.Warning
        : vscode.DiagnosticSeverity.Information;

      const diag = new vscode.Diagnostic(
        new vscode.Range(lineNum, 0, lineNum, lines[lineNum]?.length || 0),
        content,
        severity
      );
      diag.source = "ff lint";
      diagnostics.push(diag);
    }

    // Merge with existing validate diagnostics
    const existing = diagnosticCollection.get(fileUri) || [];
    const validateDiags = [...existing].filter(
      (d) => d.source === "ff validate"
    );
    diagnosticCollection.set(fileUri, [...validateDiags, ...diagnostics]);
  });
}

function runTests(fileUri: vscode.Uri) {
  const ff = getFFPath();
  const filePath = fileUri.fsPath;
  const cwd = getWorkDir(fileUri);

  // Create or reuse output channel
  const outputChannel = vscode.window.createOutputChannel("Flagfile Tests");
  outputChannel.show(true);
  outputChannel.clear();
  outputChannel.appendLine(`Running tests for ${path.basename(filePath)}...`);
  outputChannel.appendLine("");

  execFile(
    ff,
    ["test", "-f", filePath],
    { cwd },
    (err, stdout, stderr) => {
      // Strip ANSI codes for clean output
      const clean = (stdout + stderr).replace(/\x1b\[[0-9;]*m/g, "");
      outputChannel.appendLine(clean);

      if (err && err.code !== 0) {
        vscode.window.showWarningMessage("Flagfile: Some tests failed");
      } else {
        vscode.window.showInformationMessage("Flagfile: All tests passed");
      }
    }
  );
}
