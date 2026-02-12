" Vim syntax file
" Language:    Flagfile
" Maintainer:  flagfile
" Last Change: 2026-02-12

if exists('b:current_syntax')
  finish
endif

" ─── Comments ─────────────────────────────────────────────────
" Line comments: // ...
syntax match flagfileLineComment "\/\/.*$" contains=flagfileTestInComment
" Block comments: /* ... */
syntax region flagfileBlockComment start="/\*" end="\*/" contains=flagfileTestInComment
" @test inside comments gets special highlighting
syntax match flagfileTestInComment "@test\>" contained

" ─── Segment definition ───────────────────────────────────────
syntax match flagfileSegmentDef "@segment\s\+\zs[a-zA-Z_][a-zA-Z0-9_-]*" contained
syntax match flagfileSegmentKeyword "@segment" nextgroup=flagfileSegmentDef skipwhite

" ─── Annotations / metadata ───────────────────────────────────
" @owner, @ticket, @description with quoted string
syntax match flagfileAnnotationString /\("[^"]*"\|'[^']*'\)/ contained
syntax match flagfileAnnotation /@owner\|@ticket\|@description/ nextgroup=flagfileAnnotationString skipwhite

" @deprecated with quoted string (shown as deprecated/strikethrough)
syntax match flagfileDeprecatedString /\("[^"]*"\|'[^']*'\)/ contained
syntax match flagfileDeprecated /@deprecated/ nextgroup=flagfileDeprecatedString skipwhite

" @expires with date
syntax match flagfileExpiresDate /\d\{4}-\d\{2}-\d\{2}/ contained
syntax match flagfileExpires "@expires" nextgroup=flagfileExpiresDate skipwhite

" @type with known types
syntax match flagfileTypeValue /\<\%(release\|experiment\|ops\|permission\|variant\)\>/ contained
syntax match flagfileType "@type" nextgroup=flagfileTypeValue skipwhite

" @requires with flag name
syntax match flagfileRequiresFlag /FF[-_][a-zA-Z0-9_-]\+/ contained
syntax match flagfileRequires "@requires" nextgroup=flagfileRequiresFlag skipwhite

" @test as annotation (outside comments) with assertion text
syntax match flagfileTestAssertion /.\+$/ contained
syntax match flagfileTestAnnotation "^@test" nextgroup=flagfileTestAssertion skipwhite

" @env directive
syntax match flagfileEnvName /[a-zA-Z][a-zA-Z0-9_-]*/ contained
syntax match flagfileEnv "@env" nextgroup=flagfileEnvName skipwhite

" ─── Flag names ───────────────────────────────────────────────
syntax match flagfileFlagName "\<FF[-_][a-zA-Z0-9_-]\+\>"

" ─── Arrow operator ──────────────────────────────────────────
syntax match flagfileArrow "->"

" ─── Built-in functions ───────────────────────────────────────
syntax match flagfileFunction "\<\(lower\|upper\|LOWER\|UPPER\|now\|NOW\|coalesce\|segment\|percentage\)\ze\s*("

" ─── JSON blocks ──────────────────────────────────────────────
syntax region flagfileJsonBlock matchgroup=flagfileJsonFunc start="\<json\s*(" end=")" contains=flagfileJsonString,flagfileJsonNumber,flagfileJsonBool,flagfileJsonBrace,flagfileJsonBracket
syntax match flagfileJsonString /"[^"]*"/ contained containedin=flagfileJsonBlock,flagfileJsonBrace,flagfileJsonBracket
syntax match flagfileJsonNumber /-\?\d\+\(\.\d\+\)\?\([eE][+-]\?\d\+\)\?/ contained containedin=flagfileJsonBlock,flagfileJsonBrace,flagfileJsonBracket
syntax match flagfileJsonBool /\<\(true\|false\|null\)\>/ contained containedin=flagfileJsonBlock,flagfileJsonBrace,flagfileJsonBracket
syntax region flagfileJsonBrace matchgroup=flagfileJsonDelim start="{" end="}" contained containedin=flagfileJsonBlock,flagfileJsonBrace contains=flagfileJsonString,flagfileJsonNumber,flagfileJsonBool,flagfileJsonBrace,flagfileJsonBracket
syntax region flagfileJsonBracket matchgroup=flagfileJsonDelim start="\[" end="\]" contained containedin=flagfileJsonBlock,flagfileJsonBrace contains=flagfileJsonString,flagfileJsonNumber,flagfileJsonBool,flagfileJsonBrace,flagfileJsonBracket

" ─── Regex literals ───────────────────────────────────────────
" Match regex after ~ or !~ operators (e.g. ~ /pattern/ )
syntax region flagfileRegex start="\%(!\?[~]\s*\)\@<=/" skip="\\\\/" end="/" oneline

" ─── Strings ──────────────────────────────────────────────────
syntax region flagfileString start=+"+ skip=+\\\\"+ end=+"+ contains=flagfileEscape
syntax region flagfileString start=+'+ skip=+\\\\'+ end=+'+ contains=flagfileEscape
syntax match flagfileEscape /\\./ contained

" ─── Date and time literals ───────────────────────────────────
" Datetime must come before date (longer match first)
syntax match flagfileDatetime "\<\d\{4}-\d\{2}-\d\{2}T\d\{2}:\d\{2}:\d\{2}Z\?\>"
syntax match flagfileDate "\<\d\{4}-\d\{2}-\d\{2}\>"

" ─── Semver ───────────────────────────────────────────────────
syntax match flagfileSemver "\<\d\+\.\d\+\.\d\+\>"

" ─── Percentage ───────────────────────────────────────────────
syntax match flagfilePercent "\<\d\+\(\.\d\+\)\?%" contains=flagfilePercentSign
syntax match flagfilePercentSign "%" contained

" ─── Numbers ──────────────────────────────────────────────────
syntax match flagfileNumber "\<\d\+\(\.\d\+\)\?\>"

" ─── Booleans ─────────────────────────────────────────────────
syntax keyword flagfileBoolean true false TRUE FALSE

" ─── Operators ────────────────────────────────────────────────
" Comparison operators (longest match first)
syntax match flagfileComparisonOp "!=\|<>\|==\|<=\|>=\|<\|>\|="
" Match operators (longest match first)
syntax match flagfileMatchOp "!\^[~]"
syntax match flagfileMatchOp "![~]\$"
syntax match flagfileMatchOp "![~]"
syntax match flagfileMatchOp "\^[~]"
syntax match flagfileMatchOp "[~]\$"
syntax match flagfileMatchOp "[~]"

" ─── Keywords ─────────────────────────────────────────────────
" Logical operators
syntax keyword flagfileLogical and or AND OR
" Array operators
syntax match flagfileArrayOp "\<not\s\+in\>"
syntax keyword flagfileArrayOp in
" Negation
syntax keyword flagfileNegation not NOT
" Null checks (is null / is not null)
syntax match flagfileNullCheck "\<is\s\+not\s\+null\>"
syntax match flagfileNullCheck "\<is\s\+null\>"

" ─── Variables ────────────────────────────────────────────────
" Any identifier that doesn't start with FF- or FF_
" (lower priority — placed last so keywords/functions take precedence)
syntax match flagfileVariable "\<\([a-zA-Z_][a-zA-Z0-9_]*\)\>" containedin=NONE

" ─── Highlight links ─────────────────────────────────────────
" Comments
highlight default link flagfileLineComment  Comment
highlight default link flagfileBlockComment Comment
highlight default link flagfileTestInComment SpecialComment

" Segments
highlight default link flagfileSegmentKeyword Keyword
highlight default link flagfileSegmentDef     Type

" Annotations
highlight default link flagfileAnnotation       Keyword
highlight default link flagfileAnnotationString String
highlight default link flagfileDeprecated       WarningMsg
highlight default link flagfileDeprecatedString String
highlight default link flagfileExpires          Keyword
highlight default link flagfileExpiresDate      Constant
highlight default link flagfileType             Keyword
highlight default link flagfileTypeValue        Type
highlight default link flagfileRequires         Keyword
highlight default link flagfileRequiresFlag     Function
highlight default link flagfileTestAnnotation   Keyword
highlight default link flagfileTestAssertion    String
highlight default link flagfileEnv              Keyword
highlight default link flagfileEnvName          Constant

" Flag names
highlight default link flagfileFlagName Function

" Arrow
highlight default link flagfileArrow Operator

" Functions
highlight default link flagfileFunction Function

" JSON
highlight default link flagfileJsonFunc     Function
highlight default link flagfileJsonString   String
highlight default link flagfileJsonNumber   Number
highlight default link flagfileJsonBool     Boolean
highlight default link flagfileJsonDelim    Delimiter

" Regex
highlight default link flagfileRegex String

" Strings
highlight default link flagfileString String
highlight default link flagfileEscape SpecialChar

" Date/time
highlight default link flagfileDatetime Constant
highlight default link flagfileDate     Constant

" Semver
highlight default link flagfileSemver Constant

" Numbers and percentage
highlight default link flagfileNumber      Number
highlight default link flagfilePercent     Number
highlight default link flagfilePercentSign Operator

" Booleans
highlight default link flagfileBoolean Boolean

" Operators
highlight default link flagfileComparisonOp Operator
highlight default link flagfileMatchOp      Operator

" Keywords
highlight default link flagfileLogical  Keyword
highlight default link flagfileArrayOp  Keyword
highlight default link flagfileNegation Keyword
highlight default link flagfileNullCheck Keyword

" Variables
highlight default link flagfileVariable Identifier

let b:current_syntax = 'flagfile'
