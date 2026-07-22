let _cfg = null;
export function setConfig(cfg) {
    _cfg = cfg;
}
export function getConfig() {
    return _cfg;
}
export function isInitialized() {
    return _cfg !== null;
}
export function __resetForTests() {
    _cfg = null;
}
//# sourceMappingURL=config.js.map