" Filetype plugin for Flagfile
" Language:    Flagfile
" Maintainer:  flagfile

if exists('b:did_ftplugin')
  finish
endif
let b:did_ftplugin = 1

" Comment settings (for vim-commentary, gcc, etc.)
setlocal commentstring=//\ %s
setlocal comments=s1:/*,mb:*,ex:*/,://

" Folding on { } blocks
setlocal foldmethod=syntax

" Bracket matching
setlocal matchpairs+={:}

" Word characters — allow hyphens in flag names for `w`/`b` motions
setlocal iskeyword+=-

let b:undo_ftplugin = 'setlocal commentstring< comments< foldmethod< matchpairs< iskeyword<'
