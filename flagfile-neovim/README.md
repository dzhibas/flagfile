# flagfile-neovim

Syntax highlighting for [Flagfile](https://github.com/dzhibas/flagfile) in Neovim / Vim.

## Features

- Syntax highlighting matching the VSCode extension
- Flag names (`FF-*`), segments, annotations, `@env` rules
- Metadata: `@owner`, `@ticket`, `@description`, `@type`, `@expires`, `@deprecated`, `@requires`
- `@test` annotations (in comments and standalone)
- Built-in functions: `segment()`, `percentage()`, `lower()`, `upper()`, `now()`, `coalesce()`
- JSON blocks, regex literals, strings, dates, datetimes, semver
- Comparison, match, logical, and array operators
- Comment toggling support (`gcc` with vim-commentary)
- Folding on `{ }` blocks

## Install

### lazy.nvim

```lua
{ "dzhibas/flagfile", config = function() end, ft = "flagfile" }
```

Since the plugin lives in a subdirectory, use the `dir` option pointing to the neovim plugin:

```lua
{
  dir = "flagfile-neovim",
  ft = "flagfile",
}
```

Or if installed from the repo:

```lua
{
  "dzhibas/flagfile",
  ft = "flagfile",
  config = function()
    vim.opt.runtimepath:append(vim.fn.stdpath("data") .. "/lazy/flagfile/flagfile-neovim")
  end,
}
```

### vim-plug

```vim
Plug 'dzhibas/flagfile', { 'rtp': 'flagfile-neovim' }
```

### Manual

Copy the contents of `flagfile-neovim/` into your Neovim config:

```bash
cp -r flagfile-neovim/ftdetect ~/.config/nvim/
cp -r flagfile-neovim/ftplugin ~/.config/nvim/
cp -r flagfile-neovim/syntax  ~/.config/nvim/
```

## Filetype Detection

The plugin automatically detects:

| Pattern | Example |
|---------|---------|
| `Flagfile` | Exact filename |
| `Flagfile.*` | `Flagfile.example`, `Flagfile.tests` |
| `*.flags` | `myproject.flags` |
