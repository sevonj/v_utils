# V Utils

<img width="800" height="552" alt="drums" src="https://github.com/user-attachments/assets/71b0413d-a260-443a-90de-db23df597c66" />

Miscellaneous SR2 tools.

- **Model Viewer**  
  Model viewer with miscellaneous utilities.
  Limited support for smesh and cmesh.
- **Packer**  
  CLI Packfile tool


## Developers

The project is licensed under MPL-2.0

### Continuous Integration

Pull requests are gatekept by [this workflow.](https://github.com/sevonj/v_utils/blob/master/.github/workflows/ci.yml) It will check if the code

- builds (duh)
- passes unit tests (run `cargo test`)
- has linter warnings (run `cargo clippy`)
- is formatted (run `cargo fmt`)
