use anyhow::Result;
use serde_json::{Map, Value};
use crate::prompts::{
    BoxFuture, McpPrompt, PromptArg, PromptMessage, PromptRegistration, Role,
};

pub struct CreateDocsPrompt;

impl McpPrompt for CreateDocsPrompt {
    fn name(&self) -> &'static str {
        "create_docs"   // unique identifier
    }

    fn description(&self) -> Option<&'static str> {
        Some("Create project documentation")
    }

    fn arguments(&self) -> Vec<PromptArg> {
        vec![]
    }

    fn get(&self, _args: Map<String, Value>) -> BoxFuture<'_, Result<Vec<PromptMessage>>> {
        Box::pin(async move {
            Ok(vec![PromptMessage {
                role: Role::User,
                text: format!("Read 3 documentation files (*.md) from the project - if they exist.\n
                Analyse the project code, paying attention to code style, architecture, stack and intended usage.\n
                Create or refactor the documentation, following the documentation style already being used.\n
                If no files exist, create the following:\n
                README.md -> include a description of the project, quickstart guide and general usage and external APIs.\n
                ARCHITECTURE.md -> describe the project architecture, message formats and internal APIs\n
                CONTRIBUTING.md -> following the best open source practices, put together a guide to properly contribute to the proejct.\n
                LICENSE.md -> if no licensing is defined, create one with the AGPL3 license.\n
                README.md should contain links to the other files. All files, except LICENSE.md should have an index."),
            }])
        })
    }
}

inventory::submit! { PromptRegistration { factory: || Box::new(CreateDocsPrompt) } }
