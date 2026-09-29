import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import i18n from '../../i18n';
import { ConnectFourBoard } from '../../components/native/ConnectFourBoard';
import { emptyBoard, makeSnapshot } from '../helpers/nativeFixtures';

describe('ConnectFourBoard', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
    });

    it('renders an accessible group with seven labelled column buttons', () => {
        render(<ConnectFourBoard snapshot={makeSnapshot()} myColor="blue" canPlay onDrop={vi.fn()} />);
        const board = screen.getByRole('group', { name: 'Connect Four board' });
        const buttons = within(board).getAllByRole('button');
        expect(buttons).toHaveLength(7);
        expect(buttons[0]).toHaveAccessibleName('Column 1, 6 free slots');
        expect(buttons[6]).toHaveAccessibleName('Column 7, 6 free slots');
    });

    it('drops a disc on click and reports only the column', async () => {
        const onDrop = vi.fn();
        render(<ConnectFourBoard snapshot={makeSnapshot()} myColor="blue" canPlay onDrop={onDrop} />);
        await userEvent.click(screen.getByRole('button', { name: /Column 4/ }));
        expect(onDrop).toHaveBeenCalledTimes(1);
        expect(onDrop).toHaveBeenCalledWith(3);
    });

    it('is keyboard operable: Enter/Space, arrow keys and the number keys 1-7', async () => {
        const onDrop = vi.fn();
        render(<ConnectFourBoard snapshot={makeSnapshot()} myColor="blue" canPlay onDrop={onDrop} />);
        const user = userEvent.setup();

        await user.tab();
        expect(screen.getByRole('button', { name: /Column 1/ })).toHaveFocus();
        await user.keyboard('{ArrowRight}{ArrowRight}');
        expect(screen.getByRole('button', { name: /Column 3/ })).toHaveFocus();
        await user.keyboard('{Enter}');
        expect(onDrop).toHaveBeenLastCalledWith(2);
        await user.keyboard(' ');
        expect(onDrop).toHaveBeenLastCalledWith(2);
        await user.keyboard('{End}');
        expect(screen.getByRole('button', { name: /Column 7/ })).toHaveFocus();
        await user.keyboard('5');
        expect(onDrop).toHaveBeenLastCalledWith(4);
        expect(onDrop).toHaveBeenCalledTimes(3);
    });

    it('does nothing when it is not the local player\'s turn', async () => {
        const onDrop = vi.fn();
        render(<ConnectFourBoard snapshot={makeSnapshot()} myColor="red" canPlay={false} onDrop={onDrop} />);
        await userEvent.click(screen.getByRole('button', { name: /Column 2/ }));
        await userEvent.keyboard('3');
        expect(onDrop).not.toHaveBeenCalled();
        expect(screen.getByRole('button', { name: /Column 2/ })).toHaveAttribute('aria-disabled', 'true');
    });

    it('refuses full columns', async () => {
        const board = emptyBoard();
        for (let r = 0; r < 6; r++) board[r][2] = r % 2 === 0 ? 1 : 2;
        const onDrop = vi.fn();
        render(<ConnectFourBoard snapshot={makeSnapshot({ board, moveCount: 6 })} myColor="blue" canPlay onDrop={onDrop} />);
        const full = screen.getByRole('button', { name: 'Column 3, full' });
        expect(full).toHaveAttribute('aria-disabled', 'true');
        await userEvent.click(full);
        await userEvent.keyboard('3');
        expect(onDrop).not.toHaveBeenCalled();
        // other columns still work
        await userEvent.click(screen.getByRole('button', { name: /Column 4/ }));
        expect(onDrop).toHaveBeenCalledWith(3);
    });

    it('shows blue and red coins where the server put them and highlights the winning four', () => {
        const board = emptyBoard();
        board[0][0] = 1; board[1][0] = 1; board[2][0] = 1; board[3][0] = 1;
        board[0][1] = 2; board[1][1] = 2; board[2][1] = 2;
        const snap = makeSnapshot({
            board, status: 'FINISHED', result: 'WIN', endReason: 'CONNECT_FOUR', currentPlayerId: null,
            winnerUserId: '11111111-1111-4111-8111-111111111111',
            winningLine: [[0, 0], [1, 0], [2, 0], [3, 0]], moveCount: 7,
            lastMove: { column: 0, row: 3, playerId: null },
        });
        const { container } = render(<ConnectFourBoard snapshot={snap} myColor="blue" canPlay={false} onDrop={vi.fn()} />);
        expect(container.querySelectorAll('[data-cell-value="1"]')).toHaveLength(4);
        expect(container.querySelectorAll('[data-cell-value="2"]')).toHaveLength(3);
        expect(container.querySelectorAll('[data-winning="true"]')).toHaveLength(4);
        expect(container.querySelectorAll('svg')).toHaveLength(7); // one coin per disc
    });

    it('localises labels (German)', async () => {
        await i18n.changeLanguage('de');
        render(<ConnectFourBoard snapshot={makeSnapshot()} myColor="blue" canPlay onDrop={vi.fn()} />);
        expect(screen.getByRole('group', { name: 'Vier-Gewinnt-Spielfeld' })).toBeInTheDocument();
        expect(screen.getByRole('button', { name: 'Spalte 1, 6 freie Felder' })).toBeInTheDocument();
    });
});
