export enum TokenType {
    IDENTIFIER = 'IDENTIFIER',
    ARROW = 'ARROW',
    BOOLEAN = 'BOOLEAN',
    JSON = 'JSON',
    LEFT_BRACE = 'LEFT_BRACE',
    RIGHT_BRACE = 'RIGHT_BRACE',
    LEFT_PAREN = 'LEFT_PAREN',
    RIGHT_PAREN = 'RIGHT_PAREN',
    COMMA = 'COMMA',
    OPERATOR = 'OPERATOR',
    STRING = 'STRING',
    NUMBER = 'NUMBER',
    DATE = 'DATE',
    COMMENT = 'COMMENT',
    EOF = 'EOF',
  }
  
  export interface Token {
    type: TokenType;
    value: string;
    line: number;
    column: number;
  }
  
  export class Lexer {
    private input: string;
    private position: number = 0;
    private line: number = 1;
    private column: number = 1;
    private currentChar: string | null = null;
  
    constructor(input: string) {
      this.input = input;
      this.currentChar = this.input.length > 0 ? this.input[0] : null;
    }
  
    private advance(): void {
      this.position++;
      this.column++;
      
      if (this.position >= this.input.length) {
        this.currentChar = null;
      } else {
        this.currentChar = this.input[this.position];
        if (this.currentChar === '\n') {
          this.line++;
          this.column = 1;
        }
      }
    }
  
    private peek(offset: number = 1): string | null {
      const peekPos = this.position + offset;
      if (peekPos >= this.input.length) {
        return null;
      }
      return this.input[peekPos];
    }
  
    private skipWhitespace(): void {
      while (
        this.currentChar !== null && 
        /\s/.test(this.currentChar)
      ) {
        this.advance();
      }
    }
  
    private skipComment(): void {
      if (this.currentChar === '/' && this.peek() === '/') {
        // Single line comment
        while (this.currentChar !== null && (this.currentChar as string) !== '\n') {
          this.advance();
        }
      } else if (this.currentChar === '/' && this.peek() === '*') {
        // Multi-line comment
        this.advance(); // Skip /
        this.advance(); // Skip *
        
        while (
          this.currentChar !== null && 
          !((this.currentChar as string) === '*' && this.peek() === '/')
        ) {
          this.advance();
        }
        
        if (this.currentChar !== null) {
          this.advance(); // Skip *
          this.advance(); // Skip /
        }
      }
    }
  
    private readIdentifier(): string {
      let result = '';
      
      while (
        this.currentChar !== null && 
        /[a-zA-Z0-9_\-]/.test(this.currentChar)
      ) {
        result += this.currentChar;
        this.advance();
      }
      
      return result;
    }
  
    private readString(): string {
      let result = '';
      const quote = this.currentChar; // Save the quote character (' or ")
      this.advance(); // Skip the opening quote
      
      while (
        this.currentChar !== null && 
        this.currentChar !== quote
      ) {
        result += this.currentChar;
        this.advance();
      }
      
      this.advance(); // Skip the closing quote
      return result;
    }
  
    private readNumber(): string {
      let result = '';
      
      while (
        this.currentChar !== null && 
        /[0-9.]/.test(this.currentChar)
      ) {
        result += this.currentChar;
        this.advance();
      }
      
      return result;
    }
  
    private readDate(): string {
      let result = '';
      
      while (
        this.currentChar !== null && 
        /[0-9\-]/.test(this.currentChar)
      ) {
        result += this.currentChar;
        this.advance();
      }
      
      return result;
    }
  
    private readJson(): string {
      let result = 'json(';
      let braceCount = 0;
      let inString = false;
      let escapeNext = false;
      
      this.advance(); // Skip 'j'
      this.advance(); // Skip 's'
      this.advance(); // Skip 'o'
      this.advance(); // Skip 'n'
      this.advance(); // Skip '('
      
      while (this.currentChar !== null) {
        if (!escapeNext && this.currentChar === '"') {
          inString = !inString;
        }
        
        if (!inString) {
          if (this.currentChar === '{') braceCount++;
          if (this.currentChar === '}') braceCount--;
        }
        
        result += this.currentChar;
        
        if (!inString && braceCount === 0 && this.currentChar === ')') {
          this.advance();
          break;
        }
        
        escapeNext = !escapeNext && inString && this.currentChar === '\\';
        this.advance();
      }
      
      return result;
    }
  
    public getNextToken(): Token {
      while (this.currentChar !== null) {
        // Skip whitespace
        if (/\s/.test(this.currentChar)) {
          this.skipWhitespace();
          continue;
        }
        
        // Skip comments
        if (this.currentChar === '/' && (this.peek() === '/' || this.peek() === '*')) {
          this.skipComment();
          continue;
        }
        
        // Identifiers (including feature flag names)
        if (/[a-zA-Z_]/.test(this.currentChar)) {
          const startColumn = this.column;
          const identifier = this.readIdentifier();
          
          // Check for boolean literals
          if (identifier.toLowerCase() === 'true' || identifier.toLowerCase() === 'false') {
            return {
              type: TokenType.BOOLEAN,
              value: identifier.toLowerCase(),
              line: this.line,
              column: startColumn
            };
          }
          
          // Check for operators
          const operators = ['and', 'or', 'not', 'in'];
          if (operators.includes(identifier.toLowerCase())) {
            return {
              type: TokenType.OPERATOR,
              value: identifier.toLowerCase(),
              line: this.line,
              column: startColumn
            };
          }
          
          return {
            type: TokenType.IDENTIFIER,
            value: identifier,
            line: this.line,
            column: startColumn
          };
        }
        
        // Arrow operator
        if (this.currentChar === '-' && this.peek() === '>') {
          const startColumn = this.column;
          this.advance(); // Skip -
          this.advance(); // Skip >
          return {
            type: TokenType.ARROW,
            value: '->',
            line: this.line,
            column: startColumn
          };
        }
        
        // Braces
        if (this.currentChar === '{') {
          const startColumn = this.column;
          this.advance();
          return {
            type: TokenType.LEFT_BRACE,
            value: '{',
            line: this.line,
            column: startColumn
          };
        }
        
        if (this.currentChar === '}') {
          const startColumn = this.column;
          this.advance();
          return {
            type: TokenType.RIGHT_BRACE,
            value: '}',
            line: this.line,
            column: startColumn
          };
        }
        
        // Parentheses
        if (this.currentChar === '(') {
          const startColumn = this.column;
          this.advance();
          return {
            type: TokenType.LEFT_PAREN,
            value: '(',
            line: this.line,
            column: startColumn
          };
        }
        
        if (this.currentChar === ')') {
          const startColumn = this.column;
          this.advance();
          return {
            type: TokenType.RIGHT_PAREN,
            value: ')',
            line: this.line,
            column: startColumn
          };
        }
        
        // Comma
        if (this.currentChar === ',') {
          const startColumn = this.column;
          this.advance();
          return {
            type: TokenType.COMMA,
            value: ',',
            line: this.line,
            column: startColumn
          };
        }
        
        // Comparison operators
        if (['=', '!', '<', '>'].includes(this.currentChar)) {
          const startColumn = this.column;
          let op = this.currentChar;
          this.advance();
          
          if (this.currentChar === '=') {
            op += this.currentChar;
            this.advance();
          }
          
          return {
            type: TokenType.OPERATOR,
            value: op,
            line: this.line,
            column: startColumn
          };
        }
        
        // String literals
        if (this.currentChar === '"' || this.currentChar === "'") {
          const startColumn = this.column;
          const value = this.readString();
          return {
            type: TokenType.STRING,
            value,
            line: this.line,
            column: startColumn
          };
        }
        
        // Number literals
        if (/[0-9]/.test(this.currentChar)) {
          const startColumn = this.column;
          
          // Check if it's a date (YYYY-MM-DD format)
          const nextChars = this.input.substring(this.position, this.position + 10);
          if (/^\d{4}-\d{2}-\d{2}/.test(nextChars)) {
            const value = this.readDate();
            return {
              type: TokenType.DATE,
              value,
              line: this.line,
              column: startColumn
            };
          }
          
          const value = this.readNumber();
          return {
            type: TokenType.NUMBER,
            value,
            line: this.line,
            column: startColumn
          };
        }
        
        // JSON literals
        if (this.currentChar === 'j' && 
            this.peek(0) === 'j' && 
            this.peek(1) === 's' && 
            this.peek(2) === 'o' && 
            this.peek(3) === 'n' && 
            this.peek(4) === '(') {
          const startColumn = this.column;
          const value = this.readJson();
          return {
            type: TokenType.JSON,
            value,
            line: this.line,
            column: startColumn
          };
        }
        
        // If we get here, we have an unrecognized character
        throw new Error(`Unexpected character: '${this.currentChar}' at line ${this.line}, column ${this.column}`);
      }
      
      // End of file
      return {
        type: TokenType.EOF,
        value: '',
        line: this.line,
        column: this.column
      };
    }
  
    public tokenize(): Token[] {
      const tokens: Token[] = [];
      let token = this.getNextToken();
      
      while (token.type !== TokenType.EOF) {
        tokens.push(token);
        token = this.getNextToken();
      }
      
      tokens.push(token); // Add EOF token
      return tokens;
    }
  }