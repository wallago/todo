import { type FormEvent, useState } from "react";

interface Props {
  token: string;
  pending: number;
  onSave: (token: string) => void;
  onReset: () => void;
  onClose: () => void;
}

/** The server token, and a way to start the local copy over. */
export function TokenForm({ token, pending, onSave, onReset, onClose }: Props) {
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
