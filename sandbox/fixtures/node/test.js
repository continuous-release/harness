const assert = require('node:assert');
const { greet } = require('./index');

assert.equal(greet(), 'ok');
console.log('node fixture ok');
