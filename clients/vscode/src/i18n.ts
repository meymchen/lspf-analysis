/**
 * Display text in the reader's language.
 *
 * `vscode.l10n.t` lives in the `vscode` module, which the modules that build
 * these strings deliberately do not import — that is what lets them be tested
 * under plain Node. So the translator is installed at activation instead, and
 * until it is, every string renders as the English it was written in.
 *
 * Only text a person reads goes through here. The vocabulary the server sends
 * in `lspfAnalysis/fileHealth` and `lspfAnalysis/functionHealth` — pillar
 * names, metric names, grade words — stays English on the wire and is
 * translated at the moment it is drawn, by {@link pillarLabel} and its
 * neighbours. The server translates the same vocabulary the same way for the
 * text it renders itself.
 */

/** Translates a message, filling its `{0}`-style placeholders. */
export type Translate = (message: string, ...args: Array<string | number>) => string;

/**
 * Substitutes `{0}`, `{1}`, … in a message, as `vscode.l10n.t` does.
 *
 * A placeholder with no argument is left as written: a sentence with a
 * visible `{3}` in it is a bug that shows itself.
 */
export function fill(message: string, args: Array<string | number>): string {
    return message.replace(/\{(\d+)\}/g, (placeholder, index: string) => {
        const argument = args[Number(index)];
        return argument === undefined ? placeholder : String(argument);
    });
}

let translate: Translate | undefined;

/**
 * Installs the editor's translator, normally `vscode.l10n.t`.
 *
 * Passing `undefined` restores the English default, which is what the tests
 * use to read the source strings back.
 */
export function useTranslator(translator: Translate | undefined): void {
    translate = translator;
}

/** Translates one message. */
export const t: Translate = (message, ...args) =>
    translate ? translate(message, ...args) : fill(message, args);

/**
 * The display name of a pillar the server named.
 *
 * The cases are spelled out rather than looked up in a map so that each
 * source string is a literal argument to `t`, which is what string extraction
 * needs. A pillar this client has never heard of — a newer server — is shown
 * under the name it arrived with rather than dropped.
 */
export function pillarLabel(name: string): string {
    switch (name) {
        case 'control flow':
            return t('control flow');
        case 'size':
            return t('size');
        case 'vocabulary load':
            return t('vocabulary load');
        case 'interface':
            return t('interface');
        default:
            return name;
    }
}

/** The display name of a metric the server named. */
export function measureLabel(name: string): string {
    switch (name) {
        case 'cognitive complexity':
            return t('cognitive complexity');
        case 'cyclomatic complexity':
            return t('cyclomatic complexity');
        case 'statements':
            return t('statements');
        case 'working memory':
            return t('working memory');
        case 'Halstead difficulty':
            return t('Halstead difficulty');
        case 'parameters':
            return t('parameters');
        default:
            return name;
    }
}

/**
 * The name a function is shown under.
 *
 * `<anonymous>` is the server's stand-in for a function with no name, and is
 * the one name here that is ours to say rather than the reader's.
 */
export function functionLabel(name: string): string {
    return name === '<anonymous>' ? t('<anonymous>') : name;
}

/** The display name of a band. */
export function gradeLabel(grade: string): string {
    switch (grade) {
        case 'excellent':
            return t('excellent');
        case 'good':
            return t('good');
        case 'fair':
            return t('fair');
        case 'poor':
            return t('poor');
        default:
            return grade;
    }
}
