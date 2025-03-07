/**
 * Parser for a simple filter language with support for:
 * - Comparison operators: ==, !=, >, <, >=, <=, in, not in
 * - Logical operators: and, or, not
 * - Data types: strings, numbers, dates, arrays
 */

class FilterParser {
    constructor() {
        // Token types
        this.TOKEN_TYPES = {
            IDENTIFIER: 'IDENTIFIER',
            OPERATOR: 'OPERATOR',
            STRING: 'STRING',
            NUMBER: 'NUMBER',
            DATE: 'DATE',
            ARRAY_START: 'ARRAY_START',
            ARRAY_END: 'ARRAY_END',
            COMMA: 'COMMA',
            LOGICAL: 'LOGICAL',
            EOF: 'EOF'
        };

        // Supported operators
        this.OPERATORS = ['==', '!=', '>', '<', '>=', '<=', 'in', 'not in'];

        // Logical operators
        this.LOGICAL_OPERATORS = ['and', 'or', 'not'];
    }

    /**
     * Tokenize the input string into tokens
     * @param {string} input - The filter expression to tokenize
     * @returns {Array} - Array of tokens
     */
    tokenize(input) {
        const tokens = [];
        let pos = 0;

        while (pos < input.length) {
            let char = input[pos];

            // Skip whitespace
            if (/\s/.test(char)) {
                pos++;
                continue;
            }

            // Identifiers (field names)
            if (/[a-zA-Z_]/.test(char)) {
                let identifier = '';
                while (pos < input.length && /[a-zA-Z0-9_]/.test(input[pos])) {
                    identifier += input[pos++];
                }

                // Check if it's a logical operator
                if (this.LOGICAL_OPERATORS.includes(identifier)) {
                    tokens.push({ type: this.TOKEN_TYPES.LOGICAL, value: identifier });
                }
                // Check if it's part of "not in" operator
                else if (identifier === 'not' && input.substr(pos).trim().startsWith('in')) {
                    // Skip whitespace
                    while (pos < input.length && /\s/.test(input[pos])) {
                        pos++;
                    }
                    // Skip "in"
                    pos += 2;
                    tokens.push({ type: this.TOKEN_TYPES.OPERATOR, value: 'not in' });
                }
                // Check if it's an "in" operator
                else if (identifier === 'in') {
                    tokens.push({ type: this.TOKEN_TYPES.OPERATOR, value: 'in' });
                }
                else {
                    tokens.push({ type: this.TOKEN_TYPES.IDENTIFIER, value: identifier });
                }
                continue;
            }

            // Check for dates (YYYY-MM-DD format) BEFORE numbers
            if (/\d/.test(char) &&
                pos + 9 < input.length &&
                input.substr(pos, 10).match(/^\d{4}-\d{2}-\d{2}$/)) {
                const dateStr = input.substr(pos, 10);
                tokens.push({ type: this.TOKEN_TYPES.DATE, value: new Date(dateStr) });
                pos += 10;
                continue;
            }

            // Numbers - only process after checking for dates
            if (/[0-9]/.test(char)) {
                let number = '';
                while (pos < input.length && /[0-9.]/.test(input[pos])) {
                    number += input[pos++];
                }
                tokens.push({ type: this.TOKEN_TYPES.NUMBER, value: parseFloat(number) });
                continue;
            }

            // Strings (quoted)
            if (char === '"' || char === "'") {
                const quote = char;
                let string = '';
                pos++; // Skip opening quote
                while (pos < input.length && input[pos] !== quote) {
                    string += input[pos++];
                }
                pos++; // Skip closing quote
                tokens.push({ type: this.TOKEN_TYPES.STRING, value: string });
                continue;
            }

            // Operators
            if (char === '=' || char === '!' || char === '>' || char === '<') {
                let operator = char;
                pos++;

                // Check for double-character operators (==, !=, >=, <=)
                if (pos < input.length && input[pos] === '=') {
                    operator += '=';
                    pos++;
                }

                tokens.push({ type: this.TOKEN_TYPES.OPERATOR, value: operator });
                continue;
            }

            // Array syntax
            if (char === '(') {
                tokens.push({ type: this.TOKEN_TYPES.ARRAY_START, value: '(' });
                pos++;
                continue;
            }

            if (char === ')') {
                tokens.push({ type: this.TOKEN_TYPES.ARRAY_END, value: ')' });
                pos++;
                continue;
            }

            if (char === ',') {
                tokens.push({ type: this.TOKEN_TYPES.COMMA, value: ',' });
                pos++;
                continue;
            }

            // If we get here, we encountered an unexpected character
            throw new Error(`Unexpected character ${char} at position ${pos}`);
        }

        tokens.push({ type: this.TOKEN_TYPES.EOF, value: 'EOF' });
        return tokens;
    }

    /**
     * Parse the tokenized input into an abstract syntax tree
     * @param {Array} tokens - Array of tokens
     * @returns {Object} - AST representation of the filter
     */
    parse(tokens) {
        let current = 0;

        // Helper function to consume a token and advance
        const consume = (type, errorMessage) => {
            const token = tokens[current];
            if (token.type !== type) {
                throw new Error(errorMessage || `Expected ${type} but got ${token.type} (value: ${token.value})`);
            }
            current++;
            return token;
        };

        // Helper function to peek at the current token
        const peek = () => tokens[current];

        // Parse expression - entry point for parsing
        const parseExpression = () => {
            return parseLogicalExpression();
        };

        // Parse a logical expression (AND, OR)
        const parseLogicalExpression = () => {
            let left = parseComparisonExpression();

            while (current < tokens.length - 1 &&
            peek().type === this.TOKEN_TYPES.LOGICAL &&
            (peek().value === 'and' || peek().value === 'or')) {
                const operator = consume(this.TOKEN_TYPES.LOGICAL).value;
                const right = parseComparisonExpression();
                left = {
                    type: 'LogicalExpression',
                    operator,
                    left,
                    right
                };
            }

            return left;
        };

        // Parse a comparison expression (==, !=, >, <, >=, <=, in, not in)
        const parseComparisonExpression = () => {
            // Handle NOT operator
            if (peek().type === this.TOKEN_TYPES.LOGICAL && peek().value === 'not') {
                consume(this.TOKEN_TYPES.LOGICAL);
                const expression = parseComparisonExpression();
                return {
                    type: 'UnaryExpression',
                    operator: 'not',
                    argument: expression
                };
            }

            const identifier = consume(this.TOKEN_TYPES.IDENTIFIER).value;
            const operator = consume(this.TOKEN_TYPES.OPERATOR).value;

            // Handle different value types based on the next token
            let value;
            const nextToken = peek();

            if (nextToken.type === this.TOKEN_TYPES.ARRAY_START) {
                // Parse array for 'in' and 'not in' operators
                consume(this.TOKEN_TYPES.ARRAY_START);
                value = [];

                // Empty array check
                if (peek().type === this.TOKEN_TYPES.ARRAY_END) {
                    consume(this.TOKEN_TYPES.ARRAY_END);
                    return {
                        type: 'ComparisonExpression',
                        field: identifier,
                        operator,
                        value
                    };
                }

                // Parse comma-separated values
                while (true) {
                    const token = peek();

                    if (token.type === this.TOKEN_TYPES.NUMBER) {
                        value.push(consume(this.TOKEN_TYPES.NUMBER).value);
                    } else if (token.type === this.TOKEN_TYPES.STRING) {
                        value.push(consume(this.TOKEN_TYPES.STRING).value);
                    } else if (token.type === this.TOKEN_TYPES.DATE) {
                        value.push(consume(this.TOKEN_TYPES.DATE).value);
                    } else {
                        throw new Error(`Unexpected token in array: ${token.type}`);
                    }

                    if (peek().type === this.TOKEN_TYPES.ARRAY_END) {
                        break;
                    }

                    consume(this.TOKEN_TYPES.COMMA, 'Expected comma in array');
                }

                consume(this.TOKEN_TYPES.ARRAY_END);
            } else if (nextToken.type === this.TOKEN_TYPES.NUMBER) {
                value = consume(this.TOKEN_TYPES.NUMBER).value;
            } else if (nextToken.type === this.TOKEN_TYPES.STRING) {
                value = consume(this.TOKEN_TYPES.STRING).value;
            } else if (nextToken.type === this.TOKEN_TYPES.DATE) {
                value = consume(this.TOKEN_TYPES.DATE).value;
            } else {
                throw new Error(`Unexpected token after operator: ${nextToken.type} (value: ${nextToken.value})`);
            }

            return {
                type: 'ComparisonExpression',
                field: identifier,
                operator,
                value
            };
        };

        // Start parsing from the top-level expression
        return parseExpression();
    }

    /**
     * Evaluate an AST node against the provided data
     * @param {Object} node - AST node
     * @param {Object} data - Data to evaluate against
     * @returns {boolean} - Result of the evaluation
     */
    evaluate(node, data) {
        switch (node.type) {
            case 'LogicalExpression': {
                const left = this.evaluate(node.left, data);

                // Short-circuit evaluation
                if (node.operator === 'and' && !left) return false;
                if (node.operator === 'or' && left) return true;

                const right = this.evaluate(node.right, data);

                if (node.operator === 'and') return left && right;
                if (node.operator === 'or') return left || right;

                throw new Error(`Unknown logical operator: ${node.operator}`);
            }

            case 'UnaryExpression': {
                if (node.operator === 'not') {
                    return !this.evaluate(node.argument, data);
                }
                throw new Error(`Unknown unary operator: ${node.operator}`);
            }

            case 'ComparisonExpression': {
                const fieldValue = data[node.field];
                const comparisonValue = node.value;

                // Handle undefined fields
                if (fieldValue === undefined) {
                    return false;
                }

                switch (node.operator) {
                    case '==':
                        return fieldValue == comparisonValue;
                    case '!=':
                        return fieldValue != comparisonValue;
                    case '>':
                        return fieldValue > comparisonValue;
                    case '<':
                        return fieldValue < comparisonValue;
                    case '>=':
                        return fieldValue >= comparisonValue;
                    case '<=':
                        return fieldValue <= comparisonValue;
                    case 'in':
                        return Array.isArray(comparisonValue) && comparisonValue.includes(fieldValue);
                    case 'not in':
                        return Array.isArray(comparisonValue) && !comparisonValue.includes(fieldValue);
                    default:
                        throw new Error(`Unknown comparison operator: ${node.operator}`);
                }
            }

            default:
                throw new Error(`Unknown node type: ${node.type}`);
        }
    }

    /**
     * Parse and evaluate a filter expression against data
     * @param {string} expression - Filter expression to evaluate
     * @param {Object} data - Data to evaluate against
     * @returns {boolean} - Result of the evaluation
     */
    evaluateExpression(expression, data) {
        const tokens = this.tokenize(expression);
        const ast = this.parse(tokens);
        return this.evaluate(ast, data);
    }
}

// Example usage
const filterParser = new FilterParser();

// Helper function to test expressions
function testFilter(expression, data) {
    try {
        console.log(`Expression: ${expression}`);
        console.log(`Data: ${JSON.stringify(data, (key, value) => {
            if (value instanceof Date) return value.toISOString();
            return value;
        })}`);

        // Show tokenization
        const tokens = filterParser.tokenize(expression);
        console.log(`Tokens: ${JSON.stringify(tokens, (key, value) => {
            if (value instanceof Date) return value.toISOString();
            return value;
        }, 2)}`);

        // Show AST
        const ast = filterParser.parse(tokens);
        console.log(`AST: ${JSON.stringify(ast, null, 2)}`);

        const result = filterParser.evaluateExpression(expression, data);
        console.log(`Result: ${result}`);
        console.log('---');
        return result;
    } catch (error) {
        console.error(`Error evaluating "${expression}": ${error.message}`);
        return null;
    }
}

// Test examples
const testData = {
    country: "NL",
    created: new Date("2024-03-01"),
    userId: 123456
};

testFilter('country == "NL" and created > 2024-02-15 and not userId in (122133, 122132323, 2323423)', testData);