import { expect, test, type Page } from '@playwright/test';

async function expectEqualButtonBoxes(page: Page) {
  const boxes = await page
    .locator('.global-language-switch')
    .first()
    .locator('button')
    .evaluateAll((buttons: HTMLButtonElement[]) =>
      buttons.map((button: HTMLButtonElement) => {
        const rect = button.getBoundingClientRect();
        return { width: Math.round(rect.width), height: Math.round(rect.height) };
      }),
    );
  expect(new Set(boxes.map((box: { width: number; height: number }) => box.width)).size).toBe(1);
  expect(new Set(boxes.map((box: { width: number; height: number }) => box.height)).size).toBe(1);
}

async function searchFor(page: Page, query: string) {
  await page.getByRole('button', { name: 'Search' }).click();
  const input = page.getByPlaceholder('Search');
  await input.fill(query);
  await expect(page.getByText(query).first()).toBeVisible();
  await page.keyboard.press('Escape');
}

test('header language switcher persists and disables unsupported page languages', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/lib/cityjson-lib/');
  await expect(page.getByRole('heading', { name: 'cityjson-lib' })).toBeVisible();
  await expectEqualButtonBoxes(page);

  await page.getByRole('button', { name: 'Python examples' }).click();
  await expect(page.locator('[data-language-panel="python"]').first()).toBeVisible();
  await expect(page.getByRole('button', { name: 'Python examples' }).first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );

  await page.goto('/index/cityjson-index/');
  await expect(page.getByRole('button', { name: 'Python examples' }).first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('[data-language-panel="python"]').first()).toBeVisible();
  await expect(page.getByRole('button', { name: 'C++ examples' }).first()).toBeDisabled();
  await expect(page.getByRole('button', { name: 'WASM examples' }).first()).toBeDisabled();
});

test('API symbol links point to local generated Starlight reference pages', async ({ page }) => {
  await page.goto('/lib/cityjson-lib/');
  await page.getByRole('button', { name: 'Python examples' }).click();

  const pythonSymbol = page.locator('a.api-symbol', { hasText: 'CityModel.parse_document_bytes' }).first();
  await expect(pythonSymbol).toHaveAttribute('href', /\/reference\/cityjson-lib\/python\//);
  await pythonSymbol.click();
  await expect(page).toHaveURL(/\/reference\/cityjson-lib\/python\/#method-citymodel-parse_document_bytes$/);
  await expect(page.getByRole('heading', { name: 'CityModel.parse_document_bytes' })).toBeVisible();

  await page.goto('/lib/cityjson-lib/');
  await page.getByRole('button', { name: 'Rust examples' }).click();
  const rustSymbol = page.locator('a.api-symbol', { hasText: 'query::summary' }).first();
  await expect(rustSymbol).toHaveAttribute('href', /\/reference\/cityjson-lib\/rust\//);
  await expect(page.locator('a.api-symbol', { hasText: 'open' })).toHaveCount(0);
});

test('search finds generated API symbols and guide/spec content', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'cityjson-rs documentation' })).toBeVisible();

  for (const query of [
    'CityModel.parse_document_bytes',
    'CityIndex::reindex',
    'cityjson_lib::Model::parse_document',
    'parse_document_summary',
    'Arrow IPC layout',
  ]) {
    await searchFor(page, query);
  }
});
