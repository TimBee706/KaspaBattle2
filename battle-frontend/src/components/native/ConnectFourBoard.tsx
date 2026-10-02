import { useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConnectFourCell, type CellValue } from './ConnectFourCell';
import {
    CONNECT_FOUR_COLUMNS,
    CONNECT_FOUR_ROWS,
    isColumnFull,
    isWinningCell,
    lowestFreeRow,
    type BoardView,
} from '../../domain/nativeGame';

interface Props {
    snapshot: BoardView;
    /** color of the local player (for the hover preview); null for spectators */
    myColor: 'blue' | 'red' | null;
    /** true when the local player may drop a disc right now */
    canPlay: boolean;
    onDrop: (column: number) => void;
}

/**
 * Responsive, keyboard- and mouse-operable board.
 *
 * Each column is one `<button>`: Tab moves between columns, Enter/Space drops, ←/→/Home/End move
 * the focus and the keys 1–7 drop directly. The disc row is decided by the server.
 */
export function ConnectFourBoard({ snapshot, myColor, canPlay, onDrop }: Props) {
    const { t } = useTranslation();
    const [hovered, setHovered] = useState<number | null>(null);
    const buttons = useRef<Array<HTMLButtonElement | null>>([]);
    const finished = snapshot.status === 'FINISHED';

    const columns = useMemo(() => Array.from({ length: CONNECT_FOUR_COLUMNS }, (_, c) => c), []);
    // Render top row first.
    const rowsTopDown = useMemo(() => Array.from({ length: CONNECT_FOUR_ROWS }, (_, r) => CONNECT_FOUR_ROWS - 1 - r), []);

    const focusColumn = (column: number) => {
        const next = Math.max(0, Math.min(CONNECT_FOUR_COLUMNS - 1, column));
        buttons.current[next]?.focus();
    };

    const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, column: number) => {
        switch (event.key) {
            case 'ArrowLeft': event.preventDefault(); focusColumn(column - 1); break;
            case 'ArrowRight': event.preventDefault(); focusColumn(column + 1); break;
            case 'Home': event.preventDefault(); focusColumn(0); break;
            case 'End': event.preventDefault(); focusColumn(CONNECT_FOUR_COLUMNS - 1); break;
            default:
                if (/^[1-7]$/.test(event.key)) {
                    event.preventDefault();
                    const target = Number(event.key) - 1;
                    focusColumn(target);
                    if (canPlay && !isColumnFull(snapshot.board, target)) onDrop(target);
                }
        }
    };

    return (
        <div
            role="group"
            aria-label={t('native.board.label')}
            className="w-full max-w-xl mx-auto rounded-2xl border border-kaspa-primary/20 bg-kaspa-surface p-2 sm:p-3 shadow-glow-primary"
        >
            <div className="grid grid-cols-7 gap-1.5 sm:gap-2">
                {columns.map((column) => {
                    const full = isColumnFull(snapshot.board, column);
                    const free = snapshot.rows - snapshot.board.filter((row) => row[column] !== 0).length;
                    const disabled = !canPlay || full;
                    const landing = lowestFreeRow(snapshot.board, column);
                    const showPreview = canPlay && !full && hovered === column;
                    return (
                        <button
                            key={column}
                            ref={(el) => { buttons.current[column] = el; }}
                            type="button"
                            aria-label={
                                full
                                    ? t('native.board.column_full', { column: column + 1 })
                                    : t('native.board.column', { column: column + 1, free })
                            }
                            aria-disabled={disabled}
                            data-column={column}
                            onClick={() => { if (!disabled) onDrop(column); }}
                            onKeyDown={(e) => onKeyDown(e, column)}
                            onMouseEnter={() => setHovered(column)}
                            onMouseLeave={() => setHovered((h) => (h === column ? null : h))}
                            onFocus={() => setHovered(column)}
                            onBlur={() => setHovered((h) => (h === column ? null : h))}
                            className={[
                                'flex flex-col gap-1.5 sm:gap-2 rounded-xl p-0.5 sm:p-1 transition-colors',
                                'focus-visible:outline-none focus-visible:shadow-focus-ring',
                                disabled ? 'cursor-not-allowed' : 'cursor-pointer hover:bg-kaspa-primary/10',
                                hovered === column && !disabled ? 'bg-kaspa-primary/10' : '',
                            ].join(' ')}
                        >
                            {rowsTopDown.map((row) => {
                                const value = (snapshot.board[row]?.[column] ?? 0) as CellValue;
                                return (
                                    <ConnectFourCell
                                        key={row}
                                        value={value}
                                        isWinning={isWinningCell(snapshot, row, column)}
                                        isLast={snapshot.lastMove?.row === row && snapshot.lastMove?.column === column}
                                        previewColor={showPreview && row === landing ? myColor : null}
                                    />
                                );
                            })}
                        </button>
                    );
                })}
            </div>
            <p className="sr-only" aria-live="polite">
                {finished
                    ? t('native.board.sr_finished')
                    : snapshot.lastMove
                        ? t('native.board.sr_last_move', {
                            column: snapshot.lastMove.column + 1,
                            row: snapshot.lastMove.row + 1,
                        })
                        : t('native.board.sr_empty')}
            </p>
        </div>
    );
}
