import { Token, TokenType } from './lexer';

export enum NodeType {
  FEATURE_FLAG = 'FEATURE_FLAG',
  RULE = 'RULE',
  CONDITION = 'CONDITION',
  BINARY_EXPR = 'BINARY_EXPR',
  UNARY_EXPR = 'UNARY_EXPR',
  IDENTIFIER = 'IDENTIFIER',
  LITERAL = 'LITERAL',
  IN_EXPR = 'IN_EXPR',
  LIST = 'LIST',
  FUNCTION_CALL = 'FUNCTION_CALL',
}

export interface Node {
  type: NodeType;
}

export interface FeatureFlagNode extends Node {
  type: NodeType.FEATURE_FLAG;
  name: string;
  rules: RuleNode[];
  defaultValue: LiteralNode;
}

export interface RuleNode extends Node {
  type: NodeType.RULE;
  condition: Node | null;
  value: LiteralNode;
}

export interface ConditionNode extends Node {
  type: NodeType.CONDITION;
  left: Node;
  operator: string;
  right: Node;
}

export interface BinaryExprNode extends Node {
  type: NodeType.BINARY_EXPR;
  left: Node;
  operator: string;
  right: Node;
}

export interface UnaryExprNode extends Node {
  type: NodeType.UNARY_EXPR;
  operator: string;
  operand: Node;
}

export interface IdentifierNode extends Node {
  type: NodeType.IDENTIFIER;
  name: string;
}

export interface LiteralNode extends Node {
  type: NodeType.LITERAL;
  valueType: 'boolean' | 'string' | 'number' | 'date' | 'json';
  value: any;
}

export interface InExprNode extends Node {
  type: NodeType.IN_EXPR;
  left: Node;
  right: ListNode;
  negated: boolean;
}

export interface ListNode extends Node {
  type: NodeType.LIST;
  items: Node[];
}

export interface FunctionCallNode extends Node {
  type: NodeType.FUNCTION_CALL;
  name: string;
  arguments: Node[];
}

export class Parser {
  private tokens: Token[];
  private current: number = 0;

  constructor(tokens: Token[]) {
    this.tokens = tokens;
  }

  private peek(): Token {
    return this.tokens[this.current];
  }

  private previous(): Token {
    return this.tokens[this.current - 1];
  }

  private advance(): Token {
    if (!this.isAtEnd()) this.current++;
    return this.previous();
  }

  private isAtEnd(): boolean {
    return this.peek().type === TokenType.EOF;
  }

  private check(type: TokenType): boolean {
    if (this.isAtEnd()) return false;
    return this.peek().type === type;
  }

  private match(...types: TokenType[]): boolean {
    for (const type of types) {
      if (this.check(type)) {
        this.advance();
        return true;
      }
    }
    return false;
  }

  private consume(type: TokenType, message: string): Token {
    if (this.check(type)) return this.advance();
    
    const token = this.peek();
    throw new Error(`${message} at line ${token.line}, column ${token.column}`);
  }

  public parse(): FeatureFlagNode[] {
    const featureFlags: FeatureFlagNode[] = [];
    
    while (!this.isAtEnd()) {
      featureFlags.push(this.featureFlag());
    }
    
    return featureFlags;
  }

  private featureFlag(): FeatureFlagNode {
    const nameToken = this.consume(TokenType.IDENTIFIER, "Expected feature flag name");
    const name = nameToken.value;
    
    let rules: RuleNode[] = [];
    let defaultValue: LiteralNode;
    
    if (this.match(TokenType.LEFT_BRACE)) {
      // Feature flag with rules
      rules = this.rules();
      this.consume(TokenType.RIGHT_BRACE, "Expected '}' after rules");
      
      // The last rule without a condition is the default value
      const lastRule = rules[rules.length - 1];
      if (lastRule.condition === null) {
        defaultValue = lastRule.value;
        rules.pop(); // Remove the default rule from the rules list
      } else {
        // If no default rule is provided, default to false
        defaultValue = {
          type: NodeType.LITERAL,
          valueType: 'boolean',
          value: false
        };
      }
    } else {
      // Simple feature flag
      this.consume(TokenType.ARROW, "Expected '->' after feature flag name");
      defaultValue = this.literal();
    }
    
    return {
      type: NodeType.FEATURE_FLAG,
      name,
      rules,
      defaultValue
    };
  }

  private rules(): RuleNode[] {
    const rules: RuleNode[] = [];
    
    while (!this.check(TokenType.RIGHT_BRACE) && !this.isAtEnd()) {
      rules.push(this.rule());
    }
    
    return rules;
  }

  private rule(): RuleNode {
    // Check if this is a default rule (no condition)
    if (this.check(TokenType.BOOLEAN) || this.check(TokenType.JSON)) {
      const value = this.literal();
      return {
        type: NodeType.RULE,
        condition: null,
        value
      };
    }
    
    // Rule with condition
    const condition = this.expression();
    
    this.consume(TokenType.ARROW, "Expected '->' after condition");
    
    const value = this.literal();
    
    return {
      type: NodeType.RULE,
      condition,
      value
    };
  }

  private expression(): Node {
    return this.logicalOr();
  }

  private logicalOr(): BinaryExprNode {
    let expr = this.logicalAnd();
    
    while (this.match(TokenType.OPERATOR) && this.previous().value.toLowerCase() === 'or') {
      const operator = this.previous().value.toLowerCase();
      const right = this.logicalAnd();
      expr = {
        type: NodeType.BINARY_EXPR,
        left: expr,
        operator,
        right
      };
    }
    
    return expr;
  }

  private logicalAnd(): BinaryExprNode {
    let expr = this.equality();
    
    while (this.match(TokenType.OPERATOR) && this.previous().value.toLowerCase() === 'and') {
      const operator = this.previous().value.toLowerCase();
      const right = this.equality();
      expr = {
        type: NodeType.BINARY_EXPR,
        left: expr,
        operator,
        right
      };
    }
    
    return expr;
  }

  private equality(): BinaryExprNode {
    let expr = this.comparison();
    
    while (this.match(TokenType.OPERATOR) && ['==', '!='].includes(this.previous().value)) {
      const operator = this.previous().value;
      const right = this.comparison();
      expr = {
        type: NodeType.BINARY_EXPR,
        left: expr,
        operator,
        right
      };
    }
    
    return expr;
  }

  private comparison(): BinaryExprNode {
    let expr = this.inExpression();
    
    while (
      this.match(TokenType.OPERATOR) && 
      ['<', '<=', '>', '>='].includes(this.previous().value)
    ) {
      const operator = this.previous().value;
      const right = this.inExpression();
      expr = {
        type: NodeType.BINARY_EXPR,
        left: expr,
        operator,
        right
      };
    }
    
    return expr;
  }

  private inExpression(): InExprNode {
    const expr = this.unary();
    
    if (this.match(TokenType.OPERATOR) && 
        (this.previous().value.toLowerCase() === 'in' || 
         this.previous().value.toLowerCase() === 'not in')) {
      const isNegated = this.previous().value.toLowerCase() === 'not in';
      
      // If it's "not in", we've already consumed both tokens
      if (!isNegated && this.match(TokenType.OPERATOR) && this.previous().value.toLowerCase() === 'in') {
        // We're in the "in" case
      }
      
      this.consume(TokenType.LEFT_PAREN, "Expected '(' after 'in'");
      const items: Node[] = [];
      
      if (!this.check(TokenType.RIGHT_PAREN)) {
        do {
          items.push(this.primary());
        } while (this.match(TokenType.COMMA));
      }
      
      this.consume(TokenType.RIGHT_PAREN, "Expected ')' after list items");
      
      return {
        type: NodeType.IN_EXPR,
        left: expr,
        right: {
          type: NodeType.LIST,
          items
        },
        negated: isNegated
      };
    }
    
    return expr;
  }

  private unary(): UnaryExprNode {
    if (this.match(TokenType.OPERATOR) && this.previous().value.toLowerCase() === 'not') {
      const operator = this.previous().value.toLowerCase();
      const operand = this.unary();
      return {
        type: NodeType.UNARY_EXPR,
        operator,
        operand
      };
    }
    
    return this.primary();
  }

  private primary(): LiteralNode {
    if (this.match(TokenType.BOOLEAN, TokenType.STRING, TokenType.NUMBER, TokenType.DATE, TokenType.JSON)) {
      return this.createLiteralFromToken(this.previous());
    }
    
    if (this.match(TokenType.IDENTIFIER)) {
      const name = this.previous().value;
      
      // Check if this is a function call
      if (this.match(TokenType.LEFT_PAREN)) {
        const args: Node[] = [];
        
        if (!this.check(TokenType.RIGHT_PAREN)) {
          do {
            args.push(this.expression());
          } while (this.match(TokenType.COMMA));
        }
        
        this.consume(TokenType.RIGHT_PAREN, "Expected ')' after function arguments");
        
        return {
          type: NodeType.FUNCTION_CALL,
          name,
          arguments: args
        };
      }
      
      return {
        type: NodeType.IDENTIFIER,
        name
      };
    }
    
    if (this.match(TokenType.LEFT_PAREN)) {
      const expr = this.expression();
      this.consume(TokenType.RIGHT_PAREN, "Expected ')' after expression");
      return expr;
    }
    
    throw new Error(`Unexpected token: ${this.peek().value} at line ${this.peek().line}, column ${this.peek().column}`);
  }

  private literal(): LiteralNode {
    if (this.match(TokenType.BOOLEAN, TokenType.STRING, TokenType.NUMBER, TokenType.DATE, TokenType.JSON)) {
      return this.createLiteralFromToken(this.previous());
    }
    
    throw new Error(`Expected literal value at line ${this.peek().line}, column ${this.peek().column}`);
  }

  private createLiteralFromToken(token: Token): LiteralNode {
    switch (token.type) {
      case TokenType.BOOLEAN:
        return {
          type: NodeType.LITERAL,
          valueType: 'boolean',
          value: token.value.toLowerCase() === 'true'
        };
      
      case TokenType.STRING:
        return {
          type: NodeType.LITERAL,
          valueType: 'string',
          value: token.value
        };
      
      case TokenType.NUMBER:
        return {
          type: NodeType.LITERAL,
          valueType: 'number',
          value: parseFloat(token.value)
        };
      
      case TokenType.DATE:
        return {
          type: NodeType.LITERAL,
          valueType: 'date',
          value: new Date(token.value)
        };
      
      case TokenType.JSON:
        try {
          // Extract the JSON string from json(...) format
          const jsonStr = token.value.substring(5, token.value.length - 1);
          return {
            type: NodeType.LITERAL,
            valueType: 'json',
            value: JSON.parse(jsonStr)
          };
        } catch (e) {
          throw new Error(`Invalid JSON at line ${token.line}, column ${token.column}: ${e.message}`);
        }
      
      default:
        throw new Error(`Unexpected token type: ${token.type}`);
    }
  }
}