import {
    Node,
    NodeType,
    FeatureFlagNode,
    RuleNode,
    ConditionNode,
    BinaryExprNode,
    UnaryExprNode,
    IdentifierNode,
    LiteralNode,
    InExprNode,
    ListNode,
    FunctionCallNode
  } from './parser';
  
  export interface Context {
    [key: string]: any;
  }
  
  export class Evaluator {
    private context: Context;
  
    constructor(context: Context = {}) {
      this.context = context;
    }
  
    public evaluateFeatureFlags(featureFlags: FeatureFlagNode[]): Record<string, any> {
      const result: Record<string, any> = {};
      
      for (const flag of featureFlags) {
        result[flag.name] = this.evaluateFeatureFlag(flag);
      }
      
      return result;
    }
  
    public evaluateFeatureFlag(flag: FeatureFlagNode): any {
      // Evaluate each rule in order
      for (const rule of flag.rules) {
        if (rule.condition === null || this.evaluateNode(rule.condition)) {
          return this.evaluateNode(rule.value);
        }
      }
      
      // If no rule matches, return the default value
      return this.evaluateNode(flag.defaultValue);
    }
  
    private evaluateNode(node: Node): any {
      switch (node.type) {
        case NodeType.FEATURE_FLAG:
          return this.evaluateFeatureFlag(node as FeatureFlagNode);
        
        case NodeType.RULE:
          return this.evaluateRule(node as RuleNode);
        
        case NodeType.CONDITION:
          return this.evaluateCondition(node as ConditionNode);
        
        case NodeType.BINARY_EXPR:
          return this.evaluateBinaryExpr(node as BinaryExprNode);
        
        case NodeType.UNARY_EXPR:
          return this.evaluateUnaryExpr(node as UnaryExprNode);
        
        case NodeType.IDENTIFIER:
          return this.evaluateIdentifier(node as IdentifierNode);
        
        case NodeType.LITERAL:
          return this.evaluateLiteral(node as LiteralNode);
        
        case NodeType.IN_EXPR:
          return this.evaluateInExpr(node as InExprNode);
        
        case NodeType.LIST:
          return this.evaluateList(node as ListNode);
        
        case NodeType.FUNCTION_CALL:
          return this.evaluateFunctionCall(node as FunctionCallNode);
        
        default:
          throw new Error(`Unknown node type: ${node.type}`);
      }
    }
  
    private evaluateRule(rule: RuleNode): any {
      if (rule.condition === null || this.evaluateNode(rule.condition)) {
        return this.evaluateNode(rule.value);
      }
      return null;
    }
  
    private evaluateCondition(condition: ConditionNode): boolean {
      const left = this.evaluateNode(condition.left);
      const right = this.evaluateNode(condition.right);
      
      switch (condition.operator) {
        case '==':
          return left === right;
        case '!=':
          return left !== right;
        case '<':
          return left < right;
        case '<=':
          return left <= right;
        case '>':
          return left > right;
        case '>=':
          return left >= right;
        default:
          throw new Error(`Unknown operator: ${condition.operator}`);
      }
    }
  
    private evaluateBinaryExpr(expr: BinaryExprNode): any {
      const left = this.evaluateNode(expr.left);
      
      // Short-circuit evaluation for logical operators
      if (expr.operator === 'and') {
        return left ? this.evaluateNode(expr.right) : false;
      }
      
      if (expr.operator === 'or') {
        return left ? true : this.evaluateNode(expr.right);
      }
      
      const right = this.evaluateNode(expr.right);
      
      switch (expr.operator) {
        case '==':
          return left === right;
        case '!=':
          return left !== right;
        case '<':
          return left < right;
        case '<=':
          return left <= right;
        case '>':
          return left > right;
        case '>=':
          return left >= right;
        default:
          throw new Error(`Unknown operator: ${expr.operator}`);
      }
    }
  
    private evaluateUnaryExpr(expr: UnaryExprNode): any {
      const operand = this.evaluateNode(expr.operand);
      
      switch (expr.operator) {
        case 'not':
          return !operand;
        default:
          throw new Error(`Unknown operator: ${expr.operator}`);
      }
    }
  
    private evaluateIdentifier(identifier: IdentifierNode): any {
      if (identifier.name in this.context) {
        return this.context[identifier.name];
      }
      
      throw new Error(`Undefined variable: ${identifier.name}`);
    }
  
    private evaluateLiteral(literal: LiteralNode): any {
      return literal.value;
    }
  
    private evaluateInExpr(expr: InExprNode): boolean {
      const left = this.evaluateNode(expr.left);
      const list = this.evaluateNode(expr.right) as any[];
      
      const result = list.includes(left);
      return expr.negated ? !result : result;
    }
  
    private evaluateList(list: ListNode): any[] {
      return list.items.map(item => this.evaluateNode(item));
    }
  
    private evaluateFunctionCall(call: FunctionCallNode): any {
      const args = call.arguments.map(arg => this.evaluateNode(arg));
      
      switch (call.name.toLowerCase()) {
        case 'now':
          return new Date();
        default:
          throw new Error(`Unknown function: ${call.name}`);
      }
    }
  }