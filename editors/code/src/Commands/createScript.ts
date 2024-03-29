import {
  workspace as Workspace,
  window,
  commands,
  extensions,
  workspace,
} from "vscode";
import {
  existsSync,
  readFileSync,
  copyFileSync,
  writeFileSync,
  mkdirSync,
} from "fs";
import { URI } from "vscode-uri";
import { join, basename } from "path";

export function run(rootpath?: string) {
  const AuthorName: string = Workspace.getConfiguration("sourcepawn").get(
    "AuthorName"
  );
  if (!AuthorName) {
    window
      .showWarningMessage("You didn't specify an author name.", "Open Settings")
      .then((choice) => {
        if (choice === "Open Settings") {
          commands.executeCommand(
            "workbench.action.openSettings",
            "@ext:sarrus.sourcepawn-vscode"
          );
        }
      });
  }

  const GithubName: string = Workspace.getConfiguration("sourcepawn").get(
    "GithubName"
  );

  // get workspace folder
  const workspaceFolders = Workspace.workspaceFolders;
  if (!workspaceFolders) {
    window.showErrorMessage("No workspace are opened.");
    return 1;
  }

  //Select the rootpath
  if (rootpath === undefined) {
    rootpath = workspaceFolders?.[0].uri.fsPath;
  }

  const rootname = basename(rootpath);

  // create a scripting folder if it doesn't exist
  const scriptingFolderPath = join(rootpath, "scripting");
  if (!existsSync(scriptingFolderPath)) {
    mkdirSync(scriptingFolderPath);
  }

  // Check if file already exists
  const scriptFileName: string = rootname + ".sp";
  const scriptFilePath = join(rootpath, "scripting", scriptFileName);
  if (existsSync(scriptFilePath)) {
    window.showErrorMessage(scriptFileName + " already exists, aborting.");
    return 2;
  }
  const myExtDir: string = extensions.getExtension("Sarrus.sourcepawn-vscode")
    .extensionPath;
  const tasksTemplatesPath: string = join(
    myExtDir,
    "templates/plugin_template.sp"
  );
  copyFileSync(tasksTemplatesPath, scriptFilePath);

  // Replace placeholders
  try {
    const data = readFileSync(scriptFilePath, "utf8");
    let result = data.replace(/\${AuthorName}/gm, AuthorName);
    result = result.replace(/\${plugin_name}/gm, rootname);
    result = result.replace(/\${GithubName}/gm, GithubName);
    writeFileSync(scriptFilePath, result, "utf8");
  } catch (err) {
    console.error(err);
    return 3;
  }
  workspace
    .openTextDocument(URI.file(scriptFilePath))
    .then((document) => window.showTextDocument(document));
  return 0;
}
