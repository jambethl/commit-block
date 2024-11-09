# Commit Block

Commit Block is a terminal application which allows you to block certain domains until a configured threshold of GitHub contributions has been met.

## Running
1. Add a `.env` file to the root of the project directory. Within it, include a `GITHUB_TOKEN` variable where the value is your [GitHub API token](https://github.com/settings/tokens):
```
GITHUB_TOKEN={your_token_here}
```
2. Commit Block works by modifying your `/etc/hosts` file, so the application must be run with `sudo`:
```shell
sudo cargo run
```
3. Press `c` to open the configuration panel. You can supply your contribution goal and GitHub username there. Press `tab` to toggle between fields
4. To add a new host to your blocked list, press `i` to enter edit mode. Pressing `enter` will save your changes. You can delete an existing entry by pressing `tab`. Press `esc` to exit edit mode

