# MCP_SERVER

## Architecture

This project is simple mcp server implementation. It is written in Rust + Axum. 
Besides the main mcp engine, there's 3 other areas: 
* Tools
* Resources
* Prompts
Code related to each of these, should be added in their respective folders:
* ./src/tools/
* ./src/resources/
* ./src/prompts/

It is ok if some of these folders are empty from the start.

Adding a new functionality (tool, resource or prompt) should be as simple as dropping the file with the right Rust code inside the folder where they should belong to and recompile the mcp_server code.

This requires the engine to be able to find and load the available functionalities o startup in a dynamic way.

The communication between the MCP server and the outside world happens, initially, through `stdio` and, in a later phase, through `Streamable HTTP`. We need to be able to 

MCP configuration is picked up from a .env file at the root of the repo. This file will specify:
`MCP_PORT` -> defaults to `3333` if not specified
`MCP_LISTENING_ADDRESS` -> defaults to  `127.0.0.1` if not specified
`MCP_COMMUNICATION` -> one of `stdio` or `Streamable_HTTP`

As stdout will be used as the protocol channel, we should't use `println!()`. It will break the protocol. For debug/error logging we need to use `eprintln!()` instead.

## Build Commands
- Cargo build

## Project Structure
- Documentation source files in `.md`
- Configuration in `.env`

## Style Guidelines
- Rust needs to follow best practices in redability and modularity
- Unit tests need to be in place for each section (engine and each tool)
- Use relative links for internal references
