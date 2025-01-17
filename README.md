# Commit Block

Commit Block is a terminal application which allows you to block certain domains until a configured threshold of GitHub contributions has been met.

https://github.com/user-attachments/assets/bed587cf-44e4-4457-8f93-2342538e4361

The above clip demonstrates the ability to configure a host to be blocked until the configured contribution goal (1) has been met. Upon making a pull-request, the host becomes unblocked.

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

## Project structure

### main.rs
The `main.rs` file is responsible for the business logic of the application.

### app.rs
`app.rs` is where the state of the application is stored.

### ui.rs
`ui.rs` is responsible for defining the layout of the interface and rendering the widgets.

## FAQ

**Q. I've configured a host, but I can still access the website**

A. Ensure that the configured host has its top-level domain configured, i.e, `.com`, `.org`, etc. Check that the top-level domain of the site you're accessing matches the top-level domain configured in the application. Also make sure that the contribution goal has not been met yet, as that will unblock all configured hosts

**Q. My contribution goal isn't accurate**

A. Make sure to configure the GitHub username. Do this by pressing `c` and entering the username. If the name has been configured correctly, but the goal is showing `0/n`, verify whether the GitHub token you have configured in the `.env` file is still valid. An expired token will not return correct results.

## Contributing
There are many ways to contribute to this repository, including opening issues, raising PRs, and suggesting features.

Some general guidelines for PRs:
* Include unit tests if necessary
* Write a sensible description explaining the benefits of the change
* Keep PRs small; don't mix functional changes with 'cleanups'
