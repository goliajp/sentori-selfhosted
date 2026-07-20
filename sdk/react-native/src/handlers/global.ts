import { captureError } from '../capture';

type ErrorUtilsHandler = (error: Error, isFatal?: boolean) => void;

type ErrorUtilsLike = {
  setGlobalHandler: (handler: ErrorUtilsHandler) => void;
  getGlobalHandler: () => ErrorUtilsHandler;
};

let _previous: ErrorUtilsHandler | undefined;
let _installed = false;

export const installGlobalHandler = (): void => {
  if (_installed) return;

  const utils = (globalThis as { ErrorUtils?: ErrorUtilsLike }).ErrorUtils;
  if (!utils || typeof utils.setGlobalHandler !== 'function') return;

  _installed = true;
  _previous = utils.getGlobalHandler();

  utils.setGlobalHandler((error, isFatal) => {
    try {
      captureError(error);
    } catch {
      // never throw from the global handler
    }
    if (_previous) {
      try {
        _previous(error, isFatal);
      } catch {
        // ignore previous handler error
      }
    }
  });
};
