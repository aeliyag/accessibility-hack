import * as Tesseract from "tesseract.js";

let workerPromise: Promise<Tesseract.Worker> | null = null;

function getWorker(): Promise<Tesseract.Worker> {
  if (!workerPromise) {
    // Created once and reused: re-initializing tesseract (loading the
    // language model) on every call would add a multi-second delay to each
    // hotkey press.
    workerPromise = Tesseract.createWorker("eng");
  }
  return workerPromise;
}

export async function recognizeText(image: string): Promise<string> {
  const worker = await getWorker();
  const {
    data: { text },
  } = await worker.recognize(image);
  return text;
}
