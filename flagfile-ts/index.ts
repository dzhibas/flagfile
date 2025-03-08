import { Lexer } from './lexer';
import { Parser, NodeType } from './parser';
import { Evaluator, Context } from './evaluator';

export class FeatureFlagService {
  private flags: Record<string, any> = {};
  private evaluator: Evaluator;
  
  constructor(flagFileContent: string, context: Context = {}) {
    this.evaluator = new Evaluator(context);
    this.parseAndEvaluateFlags(flagFileContent);
  }
  
  /**
   * Parse and evaluate all feature flags from the provided content
   */
  private parseAndEvaluateFlags(content: string): void {
    try {
      // Tokenize the input
      const lexer = new Lexer(content);
      const tokens = lexer.tokenize();
      
      // Parse the tokens into an AST
      const parser = new Parser(tokens);
      const featureFlags = parser.parse();
      
      // Evaluate the feature flags
      this.flags = this.evaluator.evaluateFeatureFlags(featureFlags);
    } catch (error) {
      console.error('Error parsing feature flags:', error);
      throw error;
    }
  }
  
  /**
   * Get the value of a feature flag
   */
  public getFlag<T = boolean>(flagName: string, defaultValue: T): T {
    if (flagName in this.flags) {
      return this.flags[flagName] as T;
    }
    return defaultValue;
  }
  
  /**
   * Check if a boolean feature flag is enabled
   */
  public isEnabled(flagName: string): boolean {
    return this.getFlag(flagName, false);
  }
  
  /**
   * Update the context and re-evaluate all flags
   */
  public updateContext(newContext: Context, flagFileContent?: string): void {
    this.evaluator = new Evaluator({...this.evaluator['context'], ...newContext});
    
    if (flagFileContent) {
      this.parseAndEvaluateFlags(flagFileContent);
    } else {
      // Re-evaluate all flags with the new context
      const parser = new Parser([]);
      const featureFlags = Object.entries(this.flags).map(([name, value]) => {
        return {
          type: NodeType.FEATURE_FLAG,
          name,
          rules: [],
          defaultValue: {
            type: 'LITERAL' as const,
            valueType: typeof value === 'boolean' ? 'boolean' : 'json',
            value
          }
        };
      });
      
      this.flags = this.evaluator.evaluateFeatureFlags(featureFlags);
    }
  }
  
  /**
   * Get all feature flags
   */
  public getAllFlags(): Record<string, any> {
    return {...this.flags};
  }
}

// Export other components for advanced usage
export { Lexer } from './lexer';
export { Parser } from './parser';
export { Evaluator, Context } from './evaluator';