import Database from "better-sqlite3";
import { sharedRepositoryContractTests } from "@webglobe/core/src/storage/repositoryContractTests.js";
import { BetterSqlite3Driver } from "../src/drivers/BetterSqlite3Driver.js";
import { SqlitePlacesRepository } from "../src/SqlitePlacesRepository.js";

sharedRepositoryContractTests(() => {
  const db = new Database(":memory:");
  const driver = new BetterSqlite3Driver(db);
  return new SqlitePlacesRepository(driver);
});
