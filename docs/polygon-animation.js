(() => {
  // Every final polygon is regular and uses the same side length. The slot
  // order grows around the upper point/edge so the morphs stay symmetric.
  const polygonSideLength = 25;
  const polygonBaseline = 97;
  const polygonCenterX = 50;
  const terminalShapeFraction = 0.2;
  const regularPolygonStartIndices = new Map([
    [3, 0],
    [4, 0],
    [5, 4],
    [6, 5],
    [7, 5],
    [8, 6],
  ]);
  const regularPolygonCache = new Map();
  const expandedPolygonCache = new Map();

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function easeInOut(value) {
    return 0.5 - Math.cos(Math.PI * clamp(value, 0, 1)) / 2;
  }

  function growthInsertionAfterIndex(sides) {
    return Math.floor((sides - 4) / 2);
  }

  function stageSlots(sides) {
    const slots = [
      "triangle-top",
      "triangle-bottom-right",
      "triangle-bottom-left",
    ];
    for (let nextSides = 4; nextSides <= sides; nextSides += 1) {
      slots.splice(
        growthInsertionAfterIndex(nextSides) + 1,
        0,
        `growth-${nextSides}`,
      );
    }
    return slots;
  }

  function regularPolygonPoints(sides) {
    if (regularPolygonCache.has(sides)) {
      return regularPolygonCache.get(sides);
    }
    const radius = polygonSideLength / (2 * Math.sin(Math.PI / sides));
    const startAngle =
      sides % 2 === 0 ? -Math.PI / 2 - Math.PI / sides : -Math.PI / 2;
    const points = Array.from({ length: sides }, (_, index) => {
      const angle = startAngle + (index * 2 * Math.PI) / sides;
      return [radius * Math.cos(angle), radius * Math.sin(angle)];
    });
    const bottom = Math.max(...points.map(([, y]) => y));
    const startIndex = regularPolygonStartIndices.get(sides);
    const rotated = points
      .slice(startIndex)
      .concat(points.slice(0, startIndex));
    const translated = rotated.map(([x, y]) => [
      polygonCenterX + x,
      polygonBaseline + y - bottom,
    ]);
    regularPolygonCache.set(sides, translated);
    return translated;
  }

  function expandedPolygonPoints(sides, vertexCount) {
    const cacheKey = `${sides}:${vertexCount}`;
    if (expandedPolygonCache.has(cacheKey)) {
      return expandedPolygonCache.get(cacheKey);
    }
    const expanded = stageSlots(sides).map((slot, index) => ({
      slot,
      point: regularPolygonPoints(sides)[index],
    }));
    for (let nextSides = sides + 1; nextSides <= vertexCount; nextSides += 1) {
      const insertAfter = growthInsertionAfterIndex(nextSides);
      const insertIndex = insertAfter + 1;
      const before = expanded[insertAfter].point;
      const after = expanded[insertIndex % expanded.length].point;
      const point =
        nextSides % 2 === 0
          ? [...before]
          : [(before[0] + after[0]) / 2, (before[1] + after[1]) / 2];
      expanded.splice(insertIndex, 0, {
        slot: `growth-${nextSides}`,
        point,
      });
    }
    const points = expanded.map(({ point }) => point);
    expandedPolygonCache.set(cacheKey, points);
    return points;
  }

  function interpolatePolygon(from, to, amount) {
    return from.map((point, index) => [
      point[0] + (to[index][0] - point[0]) * amount,
      point[1] + (to[index][1] - point[1]) * amount,
    ]);
  }

  function terminalPointsForProgress(vertexCount, progress) {
    const triangle = expandedPolygonPoints(3, vertexCount);
    const line = triangle.map(([x]) => [x, polygonBaseline]);
    const dot = line.map(() => [polygonCenterX, polygonBaseline]);
    if (progress < 0.5) {
      return interpolatePolygon(dot, line, easeInOut(progress * 2));
    }
    return interpolatePolygon(line, triangle, easeInOut((progress - 0.5) * 2));
  }

  function polygonPointsForProgress(layerIndex, vertexCount, progress) {
    const stagePosition = clamp(progress, 0, 1) * 5;
    const stage = Math.min(Math.floor(stagePosition), 4);
    const transition = stagePosition - stage;
    const currentSides = Math.min(3 + layerIndex, 3 + stage);
    const nextSides = Math.min(currentSides + 1, 3 + layerIndex);
    const from = expandedPolygonPoints(currentSides, vertexCount);
    if (currentSides === nextSides) {
      return from;
    }
    const to = expandedPolygonPoints(nextSides, vertexCount);
    return interpolatePolygon(from, to, easeInOut(transition));
  }

  function pathData(points) {
    return `M ${points
      .map(([x, y]) => `${x.toFixed(3)} ${y.toFixed(3)}`)
      .join(" L")} Z`;
  }

  window.downshiftPolygonAnimation = Object.freeze({
    clamp,
    easeInOut,
    pathData,
    polygonBaseline,
    polygonCenterX,
    polygonPointsForProgress,
    polygonSideLength,
    terminalPointsForProgress,
    terminalShapeFraction,
  });
})();
