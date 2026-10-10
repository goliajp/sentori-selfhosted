import { useEffect, useId, useRef, useState } from 'react';
import { Download, File, Image as ImageIcon, X } from 'lucide-react';
import { fileIdentity, selectFiles } from './file-model';
import { Button } from './controls';
import { Dialog } from './Dialog';
type Translate = (key: string, params?: Record<string, string | number>) => string;
const isImage = (file: File) => /\.(png|jpe?g|gif|webp|avif|bmp)$/i.test(file.name) || /^image\/(png|jpeg|gif|webp|avif|bmp)$/.test(file.type);
const isText = (file: File) => /^text\//.test(file.type) || /\.(txt|log|json|csv|md|map|xml|yaml|yml)$/i.test(file.name);
function ImagePreview({ file, full = false }: { file: File; full?: boolean }) {
  const ref = useRef<HTMLImageElement>(null), [failed, setFailed] = useState(false);
  useEffect(() => { const url = URL.createObjectURL(file); if(ref.current) ref.current.src = url; return () => URL.revokeObjectURL(url); }, [file]);
  return failed ? <ImageIcon size={24} /> : <img ref={ref} alt={full ? file.name : ''} className={full ? 'file-image-full' : 'file-image-thumb'} onError={() => setFailed(true)} />;
}
function TextPreview({ file, t }: { file: File; t: Translate }) {
  const [text, setText] = useState<string>(), [failed, setFailed] = useState(false);
  useEffect(() => { let current = true; file.slice(0, 65536).text().then(value => { if(current) setText(value); }, () => { if(current) setFailed(true); }); return () => { current = false; }; }, [file]);
  return failed ? <p role="alert">{t('file.readFailed')}</p> : text === undefined ? <p role="status">{t('common.loading')}</p> : <>{file.size > 65536 && <p className="field-note">{t('file.previewTruncated')}</p>}<pre className="file-text-preview">{text}</pre></>;
}
export type FileInputProps = {
  t: Translate; number: (value: number, digits?: number) => string; active?: boolean;
  onFiles?: (added: File[]) => void; onChange?: (files: File[]) => void; accept?: string; disabled?: boolean;
  multiple?: boolean; maxFiles?: number; maxFileSize?: number; preview?: boolean;
};
export function FileInput({ t, number, active = true, onFiles, onChange, accept = '', disabled = false, multiple = true, maxFiles = 10, maxFileSize, preview = true }: FileInputProps) {
  const id = useId(), input = useRef<HTMLInputElement>(null), dragDepth = useRef(0);
  const [drag, setDrag] = useState(false), [files, setFiles] = useState<File[]>([]), [errors, setErrors] = useState<string[]>([]), [opened, setOpened] = useState<File | null>(null);
  const bytes = (value: number) => value < 1024 ? `${number(value)} B` : value < 1048576 ? `${number(value / 1024, 1)} KB` : `${number(value / 1048576, 1)} MB`;
  function receive(incoming: File[]) {
    if(disabled || !incoming.length) return;
    const result = selectFiles(files, incoming, { accept, multiple, maxFiles, maxFileSize });
    setFiles(result.files); setErrors(result.rejected.map(({file,reason}) => t(`file.reject.${reason}`, {name:file.name})));
    if(result.added.length) { onFiles?.(result.added); onChange?.(result.files); }
  }
  function remove(key?: string) { const next = key ? files.filter(f => fileIdentity(f) !== key) : []; setFiles(next); setErrors([]); onChange?.(next); if(input.current) input.current.value = ''; }
  return <div className="file-input" data-disabled={disabled || undefined}>
    <div className={`file-drop${drag && !disabled ? ' dragging' : ''}`} onDragEnter={event => { event.preventDefault(); if(!disabled && ++dragDepth.current === 1) setDrag(true); }} onDragOver={event => { event.preventDefault(); event.dataTransfer.dropEffect = disabled ? 'none' : 'copy'; }} onDragLeave={event => { event.preventDefault(); if(--dragDepth.current <= 0) { dragDepth.current = 0; setDrag(false); } }} onDrop={event => { event.preventDefault(); dragDepth.current = 0; setDrag(false); receive(Array.from(event.dataTransfer.files)); }}>
      <Download size={24} aria-hidden="true" />
      <div className="file-picker-row"><Button disabled={disabled} onClick={() => input.current?.click()}>{t('file.choose')}</Button><span id={`${id}-selection`} className="file-selection-label" role="status">{files.length ? t('file.selected',{count:files.length}) : t('file.none')}</span></div>
      <input ref={input} className="sr-only" type="file" tabIndex={-1} aria-label={t('file.choose')} accept={accept || undefined} multiple={multiple} disabled={disabled} aria-describedby={`${id}-selection ${id}-hint`} onChange={event => { receive(Array.from(event.target.files ?? [])); event.target.value = ''; }} />
      <div id={`${id}-hint`} className="field-note"><p>{t('file.drop')}</p><p>{maxFileSize ? t('file.limit',{count:multiple ? maxFiles : 1,size:bytes(maxFileSize)}) : t('file.limitCount',{count:multiple ? maxFiles : 1})}{accept && <> · {t('file.accepted',{types:accept})}</>}</p></div>
    </div>
    {!!errors.length && <ul className="file-errors" role="alert">{errors.map((error,i) => <li key={i}>{error}</li>)}</ul>}
    {!!files.length && <><div className="file-list-heading"><span className="field-label">{t('file.selected',{count:files.length})}</span><Button small variant="ghost" disabled={disabled} onClick={() => remove()}>{t('common.clear')}</Button></div><ul className="file-list">{files.map(file => <li key={fileIdentity(file)}>
      {preview && (isImage(file) || isText(file)) ? <Button variant="ghost" className="file-preview-button" onClick={() => setOpened(file)} aria-label={t('file.preview',{name:file.name})}>{isImage(file) ? <ImagePreview file={file} /> : <File size={24}/>}</Button> : <span className="file-preview-placeholder"><File size={24} aria-hidden="true" /></span>}
      <div className="file-item-copy"><strong>{file.name}</strong><span>{bytes(file.size)}{preview && !isImage(file) && !isText(file) && <> · {t('file.previewUnavailable')}</>}</span></div><Button small variant="ghost" className="button-icon" disabled={disabled} onClick={() => remove(fileIdentity(file))} aria-label={t('file.removeNamed',{name:file.name})}><X size={16}/></Button>
    </li>)}</ul></>}
    <Dialog active={active} closeLabel={t('common.close')} open={!!opened} onClose={() => setOpened(null)} title={opened?.name ?? t('file.choose')} footer={<Button onClick={() => setOpened(null)}>{t('common.close')}</Button>}>
      {opened && (isImage(opened) ? <ImagePreview key={fileIdentity(opened)} file={opened} full /> : <TextPreview t={t} key={fileIdentity(opened)} file={opened} />)}
    </Dialog>
  </div>;
}
