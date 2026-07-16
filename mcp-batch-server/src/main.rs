use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CommandSpec {
    cmd: String,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RunCommandsParams {
    commands: Vec<CommandSpec>,
    #[serde(default)]
    parallel: bool,
    #[serde(default)]
    continue_on_error: bool,
}

#[derive(Debug, Serialize)]
struct CommandResult {
    cmd: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FileSpec {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WriteFilesParams {
    files: Vec<FileSpec>,
}

#[derive(Debug, Serialize)]
struct FileResult {
    path: String,
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MakeDirsParams {
    paths: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WriteTreeParams {
    root: String,
    /// JSON object where each key is a name; a string value is written as a
    /// file's content, an object value is created as a subdirectory and
    /// walked recursively.
    tree: Value,
}

async fn run_one(spec: &CommandSpec) -> CommandResult {
    let mut command = Command::new("sh");
    command.arg("-c").arg(&spec.cmd);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    match command.output().await {
        Ok(output) => CommandResult {
            cmd: spec.cmd.clone(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        Err(err) => CommandResult {
            cmd: spec.cmd.clone(),
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("failed to spawn: {err}"),
        },
    }
}

fn write_tree_node<'a>(
    base: PathBuf,
    tree: &'a Value,
    results: &'a mut Vec<FileResult>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        let Value::Object(map) = tree else {
            results.push(FileResult {
                path: base.display().to_string(),
                ok: false,
                error: Some("tree node must be a JSON object".to_string()),
            });
            return;
        };

        for (name, value) in map {
            let path = base.join(name);
            match value {
                Value::String(content) => {
                    let write_result = async {
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent).await?;
                        }
                        fs::write(&path, content).await
                    }
                    .await;

                    results.push(match write_result {
                        Ok(()) => FileResult {
                            path: path.display().to_string(),
                            ok: true,
                            error: None,
                        },
                        Err(err) => FileResult {
                            path: path.display().to_string(),
                            ok: false,
                            error: Some(err.to_string()),
                        },
                    });
                }
                Value::Object(_) => {
                    match fs::create_dir_all(&path).await {
                        Ok(()) => {
                            write_tree_node(path, value, results).await;
                        }
                        Err(err) => {
                            results.push(FileResult {
                                path: path.display().to_string(),
                                ok: false,
                                error: Some(err.to_string()),
                            });
                        }
                    }
                }
                _ => {
                    results.push(FileResult {
                        path: path.display().to_string(),
                        ok: false,
                        error: Some(
                            "tree leaves must be strings (file content) or objects (dirs)"
                                .to_string(),
                        ),
                    });
                }
            }
        }
    })
}

#[derive(Clone)]
struct BatchServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl BatchServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Run multiple shell commands in one call instead of one Bash tool call \
        per command. Set parallel=true to run them concurrently, or false (default) to run them \
        one after another. Set continue_on_error=true to keep running remaining commands after a \
        non-zero exit. Returns exit code, stdout, and stderr for every command."
    )]
    async fn run_commands(
        &self,
        Parameters(params): Parameters<RunCommandsParams>,
    ) -> String {
        let results: Vec<CommandResult> = if params.parallel {
            let futures = params.commands.iter().map(run_one);
            futures::future::join_all(futures).await
        } else {
            let mut results = Vec::with_capacity(params.commands.len());
            for spec in &params.commands {
                let result = run_one(spec).await;
                let failed = result.exit_code != 0;
                results.push(result);
                if failed && !params.continue_on_error {
                    break;
                }
            }
            results
        };

        serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("serialize error: {e}"))
    }

    #[tool(
        description = "Write multiple files in one call instead of one Write tool call per file. \
        Parent directories are created automatically. Provide each file's full path and content."
    )]
    async fn write_files(&self, Parameters(params): Parameters<WriteFilesParams>) -> String {
        let mut results = Vec::with_capacity(params.files.len());
        for file in &params.files {
            let path = Path::new(&file.path);
            let write_result = async {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::write(path, &file.content).await
            }
            .await;

            results.push(match write_result {
                Ok(()) => FileResult {
                    path: file.path.clone(),
                    ok: true,
                    error: None,
                },
                Err(err) => FileResult {
                    path: file.path.clone(),
                    ok: false,
                    error: Some(err.to_string()),
                },
            });
        }
        serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("serialize error: {e}"))
    }

    #[tool(
        description = "Create multiple directories (including any missing parents) in one call \
        instead of one command per directory."
    )]
    async fn make_dirs(&self, Parameters(params): Parameters<MakeDirsParams>) -> String {
        let mut results = Vec::with_capacity(params.paths.len());
        for path in &params.paths {
            let outcome = fs::create_dir_all(path).await;
            results.push(match outcome {
                Ok(()) => FileResult {
                    path: path.clone(),
                    ok: true,
                    error: None,
                },
                Err(err) => FileResult {
                    path: path.clone(),
                    ok: false,
                    error: Some(err.to_string()),
                },
            });
        }
        serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("serialize error: {e}"))
    }

    #[tool(
        description = "Materialize an entire file tree in one call. `tree` is a JSON object: a \
        string value is written as a file's content, an object value becomes a subdirectory \
        (walked recursively). Use this instead of many separate make_dirs/write_files calls when \
        scaffolding a project layout."
    )]
    async fn write_tree(&self, Parameters(params): Parameters<WriteTreeParams>) -> String {
        let mut results = Vec::new();
        if let Err(err) = fs::create_dir_all(&params.root).await {
            return serde_json::to_string_pretty(&[FileResult {
                path: params.root.clone(),
                ok: false,
                error: Some(err.to_string()),
            }])
            .unwrap_or_else(|e| format!("serialize error: {e}"));
        }
        write_tree_node(PathBuf::from(&params.root), &params.tree, &mut results).await;
        serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("serialize error: {e}"))
    }
}

#[tool_handler]
impl ServerHandler for BatchServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            instructions: Some(
                "Batch execution tools. Prefer these over individual Bash/Write/mkdir calls \
                whenever a task involves more than one shell command, file write, or directory: \
                run_commands (many shell commands, sequential or parallel), write_files (many \
                files at once), make_dirs (many directories at once), write_tree (an entire \
                nested file/directory tree in one call). Using these instead of repeated \
                single-purpose tool calls saves tool-call round trips."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = BatchServer::new().serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}
