# `ccx_runner`

This is a simple GUI tool for observing running CalculiX Processes.

<img width="2560" height="1440" alt="image" src="https://github.com/user-attachments/assets/65930974-8718-49ce-ac30-14b45af08b50" />


_`ccx_runner` next to PrePoMax_

## What it does
It calls CalculiX for you and captures the console output. You can then filter the output using boolean expressions (AND `&`, OR `|`). Depending on the STEP type, you can also plot the residuals graphically. It also sets the environment variables to use the number of available CPU cores.

My plan for the future is, that the runner can be used for simple preprocessing of `.inp` files, to allow for i.e. parameter sweeps.

## Features

- [x] Detect available cores and run the analysis using them
- [x] Filter CCX output using keywords and operators
- [x] Show residuals inside a plot
- [x] Display solver status inside a table
- [ ] Parametrization of input parameters

## Installation
See the releases tab to download prebuilt binaries or use

```bash
cargo install --git https://github.com/KwentiN-ui/ccx_runner_rs.git
```

to compile the newest version yourself. This will also make the tool available as `ccx_runner` in your terminal.
