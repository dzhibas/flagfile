" Filetype detection for Flagfile
" Exact filename matches
autocmd BufRead,BufNewFile Flagfile setfiletype flagfile
autocmd BufRead,BufNewFile Flagfile.* setfiletype flagfile
" Extension-based matches
autocmd BufRead,BufNewFile *.flags setfiletype flagfile
