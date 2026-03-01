const fs = require('fs');
const path = require('path');

const localesPath = path.join(__dirname, '../src/locales');
const languages = ['de', 'en'];
const mainFile = 'common.json';

function getKeys(obj, prefix = '') {
    let keys = [];
    for (const key in obj) {
        if (typeof obj[key] === 'object' && obj[key] !== null) {
            keys = keys.concat(getKeys(obj[key], prefix + key + '.'));
        } else {
            keys.push(prefix + key);
        }
    }
    return keys;
}

function checkTranslations() {
    console.log('--- i18n Audit Starting ---');

    const contents = {};
    const keySets = {};

    for (const lang of languages) {
        const filePath = path.join(localesPath, lang, mainFile);
        if (!fs.existsSync(filePath)) {
            console.error(`❌ Missing file: ${filePath}`);
            process.exit(1);
        }
        const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
        contents[lang] = content;
        keySets[lang] = new Set(getKeys(content));
    }

    let hasError = false;

    // Compare de vs en
    const deKeys = keySets['de'];
    const enKeys = keySets['en'];

    for (const key of deKeys) {
        if (!enKeys.has(key)) {
            console.error(`❌ Key "${key}" exists in DE but missing in EN`);
            hasError = true;
        }
    }

    for (const key of enKeys) {
        if (!deKeys.has(key)) {
            console.error(`❌ Key "${key}" exists in EN but missing in DE`);
            hasError = true;
        }
    }

    // Check for empty values
    for (const lang of languages) {
        const keys = getKeys(contents[lang]);
        for (const key of keys) {
            const value = key.split('.').reduce((obj, k) => obj[k], contents[lang]);
            if (!value || value.trim() === '') {
                console.error(`⚠️ Empty value for key "${key}" in ${lang}`);
                // hasError = true; // Warning only for now
            }
        }
    }

    if (hasError) {
        console.log('--- i18n Audit FAILED ---');
        process.exit(1);
    } else {
        console.log('✅ All keys match between DE and EN.');
        console.log('--- i18n Audit PASSED ---');
    }
}

checkTranslations();
