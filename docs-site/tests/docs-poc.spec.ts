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
  await expect(page.getByRole('button', { name: 'C FFI examples' }).first()).toBeDisabled();
});

test('API symbol links point to local generated Starlight reference pages', async ({ page }) => {
  await page.goto('/lib/cityjson-lib/');
  await page.getByRole('button', { name: 'Python examples' }).click();

  const pythonSymbol = page.locator('a.api-symbol', { hasText: 'CityModel.parse_document_bytes' }).first();
  await expect(pythonSymbol).toHaveAttribute('href', /\/reference\/cityjson-lib\/python\/CityModel\//i);
  await pythonSymbol.click();
  await expect(page).toHaveURL(/\/reference\/cityjson-lib\/python\/citymodel\/#method-parse_document_bytes$/);
  await expect(page.getByRole('heading', { name: 'parse_document_bytes' })).toBeVisible();

  await page.goto('/lib/cityjson-lib/');
  await page.getByRole('button', { name: 'Rust examples' }).click();
  const rustSymbol = page.locator('a.api-symbol', { hasText: 'query::summary' }).first();
  await expect(rustSymbol).toHaveAttribute('href', /\/reference\/cityjson-lib\/rust\/cityjson_lib-query\//);
  await expect(page.locator('a.api-symbol', { hasText: 'open' })).toHaveCount(0);
});

test('owner-level reference pages group methods under their owner', async ({ page }) => {
  await page.goto('/reference/cityjson-index/rust/cityindex/');
  await expect(page.getByRole('heading', { name: 'CityIndex', level: 1 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Summary', level: 2 })).toBeVisible();
  await expect(page.locator('.api-summary-table').first()).toBeVisible();
  const toc = page.locator('nav[aria-labelledby="starlight__on-this-page"]');
  await expect(toc.getByRole('link', { name: 'Instance methods', exact: true })).toHaveAttribute(
    'href',
    '#instance-methods',
  );
  await expect(toc.getByRole('link', { name: 'reindex', exact: true })).toHaveAttribute(
    'href',
    '#method-reindex',
  );
  await expect(page.getByRole('heading', { name: 'Instance methods', level: 2 }).first()).toBeVisible();
  await expect(page.getByRole('heading', { name: 'reindex', level: 3 })).toBeVisible();
  await expect(page.locator('.api-summary-table').getByRole('link', { name: 'reindex', exact: true })).toHaveAttribute(
    'href',
    '#method-reindex',
  );

  await page.goto('/reference/cityjson-lib/c/c-ffi/');
  await expect(page.getByRole('heading', { name: 'C FFI' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Standalone functions', level: 2 }).first()).toBeVisible();
  await expect(page.getByRole('heading', { name: 'cj_model_parse_document_bytes', level: 3 })).toBeVisible();
});

test('generated API hierarchy distinguishes types and method kinds', async ({ page }) => {
  await page.goto('/reference/cityjson-lib/python/module-functions/');
  const pythonModuleContents = page.locator('.api-summary-section').first();
  await expect(pythonModuleContents.locator('.api-summary-group-label').first()).toHaveText('Types');
  await expect(pythonModuleContents.getByRole('columnheader', { name: 'Classification' })).toBeVisible();
  await expect(
    page.locator('nav[aria-labelledby="starlight__on-this-page"]').getByRole('link', { name: 'reindex', exact: true }),
  ).toHaveCount(0);

  await page.goto('/reference/cityjson-lib/python/citymodel/');
  await expect(page.getByRole('heading', { name: 'Class methods', level: 2 }).first()).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Instance methods', level: 2 }).first()).toBeVisible();
  await expect(page.locator('.api-summary-section').getByRole('columnheader', { name: 'Classification' })).toHaveCount(0);
  await expect(page.locator('section[aria-labelledby="method-create"] .api-kind-label')).toHaveCount(0);
  await expect(page.locator('section[aria-labelledby="method-summary"] .api-kind-label')).toHaveCount(0);

  await page.goto('/reference/cityjson-index/rust/cityindex/');
  await expect(page.getByRole('heading', { name: 'Associated functions', level: 2 }).first()).toBeVisible();
  await expect(page.locator('section[aria-labelledby="method-open"] .api-kind-label')).toHaveCount(0);

  await page.goto('/reference/cityjson-lib/rust/transformer/#method-transform');
  await expect(page.locator('section[aria-labelledby="method-transform"] .api-kind-label')).toHaveCount(0);
  await expect(page.getByRole('main')).not.toContainText('Kind: method');

  await page.goto('/reference/cityjson-lib/cpp/model/#method-create');
  await expect(page.getByRole('heading', { name: 'Static methods', level: 2 }).first()).toBeVisible();
  await expect(page.locator('section[aria-labelledby="method-create"] .api-kind-label')).toHaveCount(0);
});

test('generated API docstrings preserve python and rust formatting', async ({ page }) => {
  await page.goto('/reference/cityjson-lib/python/module-functions/#class-citymodel');
  const cityModelSection = page.locator('section[aria-labelledby="class-citymodel"]');
  await expect(cityModelSection).toContainText('class CityModel(handle)');
  await expect(cityModelSection).not.toContainText('CityModelhandle');

  await page.goto('/reference/cityjson-lib/python/citymodel/#method-parse_document_bytes');
  const parseDocumentSection = page.locator('section[aria-labelledby="method-parse_document_bytes"]');
  await expect(parseDocumentSection.getByRole('heading', { name: 'Parameters', level: 4 })).toBeVisible();
  await expect(parseDocumentSection.locator('.api-definition-list dt code').first()).toHaveText('data');
  await expect(parseDocumentSection.getByRole('heading', { name: 'Returns', level: 4 })).toBeVisible();
  await expect(parseDocumentSection.locator('.api-definition-list').nth(1).locator('dt > code')).toHaveText('Self');

  await page.goto('/reference/cityjson-lib/rust/transformer/#method-transform');
  await expect(page.getByRole('heading', { name: 'Raises', level: 4 })).toBeVisible();
  await expect(page.locator('section[aria-labelledby="method-transform"]')).toContainText('Transform one [x, y, z] point.');
  await expect(page.locator('section[aria-labelledby="method-transform"]')).toContainText(
    'Returns an error when PROJ rejects the point or the cached transformer lock is poisoned.',
  );
});

test('search finds generated API symbols and guide/spec content', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'cityjson-rs documentation' })).toBeVisible();

  for (const query of [
    'CityModel.parse_document_bytes',
    'CityIndex::reindex',
    'cityjson_lib::Model::parse_document',
    'parse_document_summary',
    'cj_model_parse_document_bytes',
    'cityjson_lib::Model',
    'Arrow IPC layout',
  ]) {
    await searchFor(page, query);
  }
});
