
interface ToastMessage {
  id: number;
  kind: "success" | "error" | "info" | "warn";
  text: string;
}

interface Props {
  toasts: ToastMessage[];
}

function kindStyles(kind: ToastMessage["kind"]): string {
  switch (kind) {
    case "success":
      return "toast--success";
    case "error":
      return "toast--error";
    case "warn":
      return "toast--warn";
    case "info":
      return "toast--info";
    default:
      return "";
  }
}

export default function Toast({ toasts }: Props) {
  return (
    <div className="toast-stack" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast ${kindStyles(toast.kind)}`}>
          {toast.text}
        </div>
      ))}
    </div>
  );
}
