import { type FormEvent, useState } from "react";

interface Props {
  token: string;
  pending: number;
  onSave: (token: string) => void;
  onReset: () => void;
  onClose: () => void;
  /** Notifications were just allowed: post the due ones. */
  onNotify: () => void;
}

/** `Notification.permission`, or `unsupported` where there are none. */
function permission(): NotificationPermission | "unsupported" {
  return typeof Notification === "undefined" ? "unsupported" : Notification.permission;
}

/**
 * Asks for notifications. On Android they are what puts the due count on the
 * app's icon. Asking needs a tap, so it's a button, not a prompt on start.
 */
function NotifyRow({ onNotify }: { onNotify: () => void }) {
  const [state, setState] = useState(permission);
  if (state === "unsupported" || state === "granted") return null;
  if (state === "denied") {
    return (
      <p>
        Notifications are blocked: allow them in the browser's site settings to see the due count on
        the app icon.
      </p>
    );
  }
  return (
    <p>
      <button
        type="button"
        onClick={async () => {
          const answer = await Notification.requestPermission();
          setState(answer);
          if (answer === "granted") onNotify();
        }}
      >
        Show due todos as notifications
      </button>
    </p>
  );
}

/** The server token, and a way to start the local copy over. */
export function TokenForm({ token, pending, onSave, onReset, onClose, onNotify }: Props) {
  const [value, setValue] = useState(token);
  const [confirming, setConfirming] = useState(false);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSave(value.trim());
  };
  return (
    <div className="backdrop">
      <form className="dialog" role="dialog" aria-label="Server token" onSubmit={submit}>
        <h2>Server token</h2>
        <p>
          The bearer token from the server's <code>token_file</code>. It stays on this device.
        </p>
        <label>
          Token
          <input
            type="password"
            autoComplete="off"
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
        </label>
        <NotifyRow onNotify={onNotify} />
        {confirming && (
          <p className="warning">
            Forget the local copy and fetch everything again?
            {pending > 0 && ` ${pending} change(s) not sent yet will be lost.`}
          </p>
        )}
        <div className="buttons">
          <button
            type="button"
            className="danger"
            onClick={() => (confirming ? onReset() : setConfirming(true))}
          >
            {confirming ? "Yes, forget it" : "Forget local copy"}
          </button>
          <span className="grow" />
          <button type="button" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="primary">
            Save
          </button>
        </div>
      </form>
    </div>
  );
}
