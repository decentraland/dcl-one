import { type Page } from 'playwright';
import { App } from './pageObjects/App';
import { UIDesigner } from './pageObjects/UIDesigner';
import { expect, test } from './fixtures';

declare const page: Page;

const ROOT = 'MainUI';

test.describe('UI Designer empty state', () => {
  test.beforeAll(async () => {
    await App.waitUntilReady();
    await UIDesigner.open();
  });

  test('greet a scene with no GUIs with the first-run guidance', async () => {
    const empty = page.locator(UIDesigner.emptyStateSelector);
    await empty.waitFor({ state: 'attached', timeout: 10_000 });

    await expect(empty.textContent()).resolves.toContain('Start building your UI');
    await expect(
      page.locator(`${UIDesigner.emptyStateSelector} .ui-designer-empty-state-chip`).count(),
    ).resolves.toBe(2);
  });

  test('show no Nodes section when there is no GUI at all', async () => {
    await expect(UIDesigner.isSectionVisible('GUIs')).resolves.toBe(true);
    await expect(UIDesigner.isSectionVisible('Nodes')).resolves.toBe(false);
  });

  test('leave the empty state from its own call to action', async () => {
    await UIDesigner.createRootFromEmptyState();

    await expect(UIDesigner.rootNames()).resolves.toEqual([ROOT]);
    await expect(UIDesigner.isSectionVisible('Nodes')).resolves.toBe(true);
  });

  test('add the first element from the Nodes + on an empty GUI', async () => {
    await UIDesigner.addWidget('Container');

    await expect(UIDesigner.nodeLabels()).resolves.toEqual(['Container']);
  });
});

test.describe('UI Designer left panel', () => {
  test.beforeAll(async () => {
    await App.waitUntilReady();
    await UIDesigner.open();
    await UIDesigner.addWidget('Container');
    await UIDesigner.addWidget('Label');
  });

  test('name new nodes after their widget kind, numbering collisions', async () => {
    await expect(UIDesigner.nodeLabels()).resolves.toEqual(['Container', 'Container1', 'Label']);
  });

  test('show the Nodes section once the GUI has content', async () => {
    await expect(UIDesigner.isSectionVisible('GUIs')).resolves.toBe(true);
    await expect(UIDesigner.isSectionVisible('Nodes')).resolves.toBe(true);
  });

  test("filter both GUIs and Nodes from the one search box, keeping a match's ancestors", async () => {
    await UIDesigner.search('container1');

    await page
      .locator(`${UIDesigner.railSelector} .ui-designer-roots-list`)
      .waitFor({ state: 'detached', timeout: 5_000 });
    await expect(UIDesigner.nodeLabels()).resolves.toEqual(['Container', 'Container1']);

    await UIDesigner.clearSearch();
    await page
      .locator(`${UIDesigner.railSelector} .ui-designer-roots-list`)
      .waitFor({ state: 'attached', timeout: 5_000 });
  });

  test('hide the Nodes section when only a GUI matches', async () => {
    await UIDesigner.search(ROOT.toLowerCase());

    await page.locator(UIDesigner.treeSelector).waitFor({ state: 'detached', timeout: 5_000 });
    await expect(UIDesigner.rootNames()).resolves.toEqual([ROOT]);

    await UIDesigner.clearSearch();
    await page.locator(UIDesigner.treeSelector).waitFor({ state: 'attached', timeout: 5_000 });
  });

  test('hide both sections when nothing matches', async () => {
    await UIDesigner.search('nothingmatchesthis');

    await page.locator(UIDesigner.treeSelector).waitFor({ state: 'detached', timeout: 5_000 });
    await page
      .locator(`${UIDesigner.railSelector} .ui-designer-roots-list`)
      .waitFor({ state: 'detached', timeout: 5_000 });

    await UIDesigner.clearSearch();
    await page.locator(UIDesigner.treeSelector).waitFor({ state: 'attached', timeout: 5_000 });
  });

  test('gray a hidden node and keep its eye visible without hover', async () => {
    await expect(UIDesigner.isNodeHidden('Label')).resolves.toBe(false);

    await UIDesigner.toggleHidden('Label');
    await expect(UIDesigner.isNodeHidden('Label')).resolves.toBe(true);

    await UIDesigner.toggleHidden('Label');
    await expect(UIDesigner.isNodeHidden('Label')).resolves.toBe(false);
  });

  test('offer no rename on the root container, which is 1:1 with the GUI that carries the name', async () => {
    await expect(UIDesigner.contextMenuItems('Container')).resolves.toEqual([
      'Duplicate',
      'Delete',
    ]);
    await expect(UIDesigner.contextMenuItems('Container1')).resolves.toContain('Rename');
  });

  test('rename a node, reading the new label back out of the spliced, formatted, reparsed source', async () => {
    await UIDesigner.renameNode('Label', 'Score');

    await expect(UIDesigner.nodeLabels()).resolves.toEqual(['Container', 'Container1', 'Score']);
  });

  test('number a renamed node against the names already taken', async () => {
    await UIDesigner.renameNode('Score', 'Container');

    await expect(UIDesigner.nodeLabels()).resolves.toEqual([
      'Container',
      'Container1',
      'Container2',
    ]);
  });

  test('show the Position unit value in full, inside its cell', async () => {
    await UIDesigner.selectNode('Container1');
    const unit = page.locator('.ui-designer-right-rail [aria-label="X unit"]');
    await expect(
      unit
        .locator('.Option')
        .evaluate(el => el.getBoundingClientRect().width > 0 && el.scrollWidth <= el.clientWidth),
    ).resolves.toBe(true);
    await expect(unit.evaluate(el => !!el.closest('.InputContainer'))).resolves.toBe(true);
  });

  test('show every unit option label in full when the list opens', async () => {
    await page.locator('.ui-designer-right-rail [aria-label="X unit"]').click();
    const labels = page.locator('[role="listbox"] .Option');
    await labels.first().waitFor({ state: 'visible' });
    await expect(
      labels.evaluateAll(els =>
        els.map(el => [
          el.textContent,
          el.getBoundingClientRect().width > 0 && el.scrollWidth <= el.clientWidth,
        ]),
      ),
    ).resolves.toEqual([
      ['px', true],
      ['%', true],
    ]);
    await page.keyboard.press('Escape');
  });

  test('return to the empty state when the last GUI is deleted', async () => {
    await UIDesigner.removeRoot(ROOT);

    await page
      .locator(UIDesigner.emptyStateSelector)
      .waitFor({ state: 'attached', timeout: 10_000 });
    await expect(UIDesigner.rootNames()).resolves.toEqual([]);
  });
});

test.describe('UI Designer toolbar', () => {
  test.beforeAll(async () => {
    await App.waitUntilReady();
    await UIDesigner.open();
  });

  test('draw the Snow focus ring on the 2D toolbar', async () => {
    const ring = () =>
      page.evaluate(() => {
        const el = document.activeElement as HTMLElement;
        const style = getComputedStyle(el);
        return { className: el.className, outline: `${style.outlineStyle} ${style.outlineColor}` };
      });

    await page.locator('.ui-designer-toolbar .gizmo.free').click();
    await page.keyboard.press('Tab');
    await expect(ring()).resolves.toEqual({
      className: expect.stringContaining('gizmo position'),
      outline: 'solid rgb(252, 252, 252)',
    });

    await page.keyboard.press('Tab');
    await page.keyboard.press('Tab');
    await expect(ring()).resolves.toEqual({
      className: expect.stringContaining('edit-scene'),
      outline: 'solid rgb(252, 252, 252)',
    });
  });
});

test.afterAll(async () => {
  await UIDesigner.close();
});
