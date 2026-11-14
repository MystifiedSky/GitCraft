# AI setup


Open **Preferences → AI**, choose a provider or local preset, and click **Fetch Models**. Use the returned list for the default model and any per-feature overrides. You can also enter a model ID if your server doesn't publish a list. Start your local server first. Ollama defaults to `http://localhost:11434/v1`; LM Studio uses `http://localhost:1234/v1`; llama.cpp uses `http://localhost:8080/v1`. Server roots, API bases and full chat-completions URLs are accepted.

Local and custom connections can run without an API key. Save a key only when the server requires authentication; **Remove Key** deletes a previously saved key. Hosted Anthropic, OpenAI and Google connections require their provider key. Keys live in the operating system credential store. Provider endpoints, default models and feature overrides are remembered when switching providers.

**ChatGPT subscription:** Install the [Codex CLI](https://developers.openai.com/codex/cli), run `codex login` in a terminal, and choose **Sign in with ChatGPT**. In **Preferences → AI**, select **ChatGPT subscription (Codex CLI)**.

**Claude subscription:** Install [Claude Code](https://code.claude.com/docs/en/quickstart), run `claude auth login` in a terminal, and sign in with your Claude account. In **Preferences → AI**, select **Claude subscription (Claude Code)**. If you signed in with a separate `CLAUDE_CONFIG_DIR`, choose that folder in GitCraft.

Use **Find Automatically** to locate and verify the installed CLI without sending a prompt. **Fetch Models** lists choices from the signed-in CLI, and **Test Connection** sends a short test request. You can keep the CLI's default model. GitCraft runs the CLI outside the repository with tools disabled and passes the context for the helper you request. The CLI manages its own credentials and uses your account's allowance.

**Test Connection** uses the default model. Every AI helper uses the same connection; feature model overrides are optional. **Automatic** response mode tries structured JSON, JSON mode, then prompt-only JSON if the selected server explicitly rejects that output format. You can select a mode directly for servers with limited compatibility. Replies are validated before use; authentication, rate-limit and missing-model errors are reported without format retries. Compatibility retries stay on the selected endpoint.

Model quality and context capacity depend on the model you run. Conflict suggestions remain editable and require review before saving; composed commits are reviewed before creation. External coding-agent CLIs in the Agents view keep their own provider configuration.
