import { Lexer } from './lexer';
import * as fs from 'fs';
import * as path from 'path';
      
// Read content from Flagfile.example
const filePath = path.resolve(__dirname, '../Flagfile.example');
const content = fs.readFileSync(filePath, 'utf8');
      
// Tokenize the input
const lexer = new Lexer(content);
const tokens = lexer.tokenize();
console.log(tokens);